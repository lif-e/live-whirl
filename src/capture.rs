use bevy::prelude::*;
use std::sync::mpsc::Sender;

#[derive(Resource, Clone)]
pub struct FrameSender {
    pub tx: Sender<Vec<u8>>,
}

// Cross-world channel resources (Render <-> Main)
use crossbeam_channel as xchan;
#[derive(Resource, Deref)]
pub struct MainWorldReceiver(xchan::Receiver<Vec<u8>>);
#[derive(Resource, Deref)]
pub struct RenderWorldSender(xchan::Sender<Vec<u8>>);

// Handle for the offscreen render image provided by setup
#[derive(Resource, Clone)]
pub struct RenderImageHandle(pub Handle<Image>);
// Capture config mirrored into RenderApp
#[derive(Resource, Clone)]
pub struct CaptureConfig { pub width: u32, pub height: u32 }



// Image copier component (spawned in Main, extracted to Render)
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use bevy::render::{
    render_asset::RenderAssets,
    render_graph::{self, RenderGraphContext, NodeRunError, RenderLabel},
    render_resource::{
        Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, Maintain, MapMode,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
};

#[derive(Clone, Default, Resource, Deref, DerefMut)]
struct ImageCopiers(pub Vec<ImageCopier>);

#[derive(Clone, Component)]
pub struct ImageCopier {
    buffer: Buffer,
    enabled: Arc<AtomicBool>,
    pub src_image: Handle<Image>,
}

impl ImageCopier {
    pub fn new(src_image: Handle<Image>, size: Extent3d, render_device: &RenderDevice) -> Self {
        let bytes_per_pixel = 4usize; // Rgba8UnormSrgb
        let row_bytes = (size.width as usize) * bytes_per_pixel;
        let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row(row_bytes);
        let cpu_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("copy-staging"),
            size: (padded_bytes_per_row as u64) * (size.height as u64),
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer: cpu_buffer, src_image, enabled: Arc::new(AtomicBool::new(true)) }
    }
    pub fn enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }
}

fn image_copy_extract(mut commands: bevy::prelude::Commands, copiers: Extract<Query<&ImageCopier>>) {
    commands.insert_resource(ImageCopiers(copiers.iter().cloned().collect()));
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, RenderLabel)]
struct ImageCopy;

#[derive(Default)]
struct ImageCopyDriver;

impl render_graph::Node for ImageCopyDriver {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let image_copiers = world.resource::<ImageCopiers>();
        let gpu_images = world.resource::<RenderAssets<bevy::render::texture::GpuImage>>();

        for copier in image_copiers.iter() {
            if !copier.enabled() { continue; }
            let src_image = match gpu_images.get(&copier.src_image) { Some(i) => i, None => continue };

            let mut encoder = render_context
                .render_device()
                .create_command_encoder(&CommandEncoderDescriptor::default());

            // Align row size for copy
            let block_dims = src_image.texture_format.block_dimensions();
            let block_size = src_image.texture_format.block_copy_size(None).unwrap();
            let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row(
                (src_image.size.width as usize / block_dims.0 as usize) * block_size as usize,
            );

            encoder.copy_texture_to_buffer(
                src_image.texture.as_image_copy(),
                bevy::render::render_resource::TexelCopyBufferInfo {
                    buffer: &copier.buffer,
                    layout: bevy::render::render_resource::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(std::num::NonZeroU32::new(padded_bytes_per_row as u32).unwrap().into()),
                        rows_per_image: None,
                    },
                },
                src_image.size,
            );

            let render_queue = world.resource::<RenderQueue>();
            render_queue.submit(std::iter::once(encoder.finish()));
        }
        Ok(())
    }
}

// Simple GPU staging buffer state in the render world
#[derive(Resource)]
pub struct GpuCopyState { pub buffer: bevy::render::render_resource::Buffer, pub padded_bpr: usize, pub height: u32 }

// Extract the offscreen image handle from main into the RenderApp
pub fn extract_render_image_handle(
    mut commands: bevy::prelude::Commands,
    handle: Extract<Option<Res<RenderImageHandle>>>,
) {
    if let Some(h) = handle.as_deref() { commands.insert_resource(h.clone()); }
}

// Ensure the staging buffer exists and matches the requested size
pub fn ensure_gpu_copy_state(
    mut commands: bevy::prelude::Commands,
    device: Res<RenderDevice>,
    cfg: Option<Res<CaptureConfig>>,
    state: Option<Res<GpuCopyState>>,
) {
    let Some(cfg) = cfg else { return; };
    let row_bytes = (cfg.width as usize) * 4; // RGBA8
    let padded_bpr = RenderDevice::align_copy_bytes_per_row(row_bytes);
    let needed_size = (padded_bpr as u64) * (cfg.height as u64);
    let mut need_new = true;
    if let Some(s) = state.as_ref() {
        if s.padded_bpr == padded_bpr && s.height == cfg.height { need_new = false; }
    }
    if need_new {
        let buffer = device.create_buffer(&bevy::render::render_resource::BufferDescriptor {
            label: Some("frame-staging"),
            size: needed_size,
            usage: bevy::render::render_resource::BufferUsages::MAP_READ | bevy::render::render_resource::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        commands.insert_resource(GpuCopyState { buffer, padded_bpr, height: cfg.height });
    }
}

// Copy the render target texture to CPU-visible buffer and send via crossbeam
pub fn copy_and_send_frame(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    handle: Option<Res<RenderImageHandle>>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    state: Option<Res<GpuCopyState>>,
    sender: Option<Res<RenderWorldSender>>,
) {
    let (Some(h), Some(state), Some(sender)) = (handle, state, sender) else { return; };
    let Some(src) = gpu_images.get(&h.0) else { return; };

    let mut encoder = device.create_command_encoder(&bevy::render::render_resource::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        src.texture.as_image_copy(),
        bevy::render::render_resource::TexelCopyBufferInfo {
            buffer: &state.buffer,
            layout: bevy::render::render_resource::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(std::num::NonZeroU32::new(state.padded_bpr as u32).unwrap().into()),
                rows_per_image: None,
            },
        },
        src.size,
    );
    queue.submit(std::iter::once(encoder.finish()));

    let slice = state.buffer.slice(..);
    let (s, r) = xchan::bounded(1);
    slice.map_async(bevy::render::render_resource::MapMode::Read, move |res| { let _ = s.send(res); });
    device.poll(bevy::render::render_resource::Maintain::wait()).panic_on_timeout();
    if r.recv().is_ok() {
        let bytes = slice.get_mapped_range().to_vec();
        let _ = sender.send(bytes);
        state.buffer.unmap();
    }
}


fn receive_image_from_buffer(
    image_copiers: Res<ImageCopiers>,
    render_device: Res<RenderDevice>,
    sender: Res<RenderWorldSender>,
) {
    for copier in image_copiers.iter() {
        if !copier.enabled() { continue; }
        let slice = copier.buffer.slice(..);
        let (s, r) = xchan::bounded(1);



// Mirror main-world handle into render-world so we can find the GPU image
pub fn extract_render_image_handle(mut commands: bevy::prelude::Commands, handle: Option<Res<RenderImageHandle>>) {
    if let Some(h) = handle { commands.insert_resource(h.clone()); }
}

// Simple GPU staging buffer state in the render world
#[derive(Resource)]
struct GpuCopyState { buffer: bevy::render::render_resource::Buffer, padded_bpr: usize, height: u32 }

pub fn ensure_gpu_copy_state(
    mut commands: bevy::prelude::Commands,
    device: Res<RenderDevice>,
    cfg: Option<Res<CaptureConfig>>,
    state: Option<Res<GpuCopyState>>,
) {
    let Some(cfg) = cfg else { return; };
    let row_bytes = (cfg.width as usize) * 4; // RGBA8
    let padded_bpr = RenderDevice::align_copy_bytes_per_row(row_bytes);
    let needed_size = (padded_bpr as u64) * (cfg.height as u64);
    let mut need_new = true;
    if let Some(s) = state.as_ref() {
        if s.padded_bpr == padded_bpr && s.height == cfg.height { need_new = false; }
    }
    if need_new {
        let buffer = device.create_buffer(&bevy::render::render_resource::BufferDescriptor {
            label: Some("frame-staging"),
            size: needed_size,
            usage: bevy::render::render_resource::BufferUsages::MAP_READ | bevy::render::render_resource::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        commands.insert_resource(GpuCopyState { buffer, padded_bpr, height: cfg.height });
    }
}

pub fn copy_and_send_frame(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    handle: Option<Res<RenderImageHandle>>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    state: Option<Res<GpuCopyState>>,
    sender: Option<Res<RenderWorldSender>>,
) {
    let (Some(h), Some(state), Some(sender)) = (handle, state, sender) else { return; };
    let Some(src) = gpu_images.get(&h.0) else { return; };

    let mut encoder = device.create_command_encoder(&bevy::render::render_resource::CommandEncoderDescriptor::default());

    encoder.copy_texture_to_buffer(
        src.texture.as_image_copy(),
        bevy::render::render_resource::TexelCopyBufferInfo {
            buffer: &state.buffer,
            layout: bevy::render::render_resource::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(std::num::NonZeroU32::new(state.padded_bpr as u32).unwrap().into()),
                rows_per_image: None,
            },
        },
        src.size,
    );

    queue.submit(std::iter::once(encoder.finish()));

    let slice = state.buffer.slice(..);
    let (s, r) = xchan::bounded(1);
    slice.map_async(bevy::render::render_resource::MapMode::Read, move |res| { let _ = s.send(res); });
    device.poll(bevy::render::render_resource::Maintain::wait()).panic_on_timeout();
    if r.recv().is_ok() {
        let bytes = slice.get_mapped_range().to_vec();
        let _ = sender.send(bytes);
        state.buffer.unmap();
    }
}

        slice.map_async(MapMode::Read, move |res| { let _ = s.send(res); });
        render_device.poll(Maintain::wait()).panic_on_timeout();
        if r.recv().is_ok() {
            let bytes = slice.get_mapped_range().to_vec();
            let _ = sender.send(bytes);
            copier.buffer.unmap();
        }
    }
}

fn forward_frames_to_ffmpeg(
    receiver: Option<Res<MainWorldReceiver>>,
    sender: Option<Res<FrameSender>>,
    cfg: Option<Res<crate::setup::VideoExportRequest>>,
) {
    let (Some(rx), Some(sender), Some(cfg)) = (receiver, sender, cfg) else { return; };
    let row_bytes = (cfg.width as usize) * 4;
    let aligned = RenderDevice::align_copy_bytes_per_row(row_bytes);
    let tx = &sender.tx;
    // Drain all available frames and forward the last one
    let mut last: Option<Vec<u8>> = None;
    while let Ok(bytes) = rx.try_recv() { last = Some(bytes); }
    if let Some(img) = last {
        if aligned == row_bytes {
            let _ = tx.send(img);
        } else {
            // shrink rows
            let mut out = Vec::with_capacity(row_bytes * (cfg.height as usize));
            for row in img.chunks(aligned).take(cfg.height as usize) {
                out.extend_from_slice(&row[..row_bytes.min(row.len())]);
            }
            let _ = tx.send(out);
        }
    }
}


pub fn add_render_capture_systems(app: &mut App) {
    // Setup cross-world channel for image bytes
    let (s, r) = xchan::unbounded();
    app.insert_resource(MainWorldReceiver(r));

    // Mirror capture config into RenderApp if present
    if let Some(req) = app.world_mut().get_resource::<crate::setup::VideoExportRequest>().cloned() {
        app.sub_app_mut(RenderApp).insert_resource(CaptureConfig { width: req.width, height: req.height });
    }

    // RenderApp systems: extract handle, ensure staging buffer, copy+send
    app.sub_app_mut(RenderApp)
        .insert_resource(RenderWorldSender(s))
        .add_systems(ExtractSchedule, extract_render_image_handle)
        .add_systems(Render, (
            ensure_gpu_copy_state,
            copy_and_send_frame.after(RenderSet::Render),
        ));

    // Main world: forward from crossbeam receiver to ffmpeg channel
    app.add_systems(bevy::app::PostUpdate, forward_frames_to_ffmpeg);
}
