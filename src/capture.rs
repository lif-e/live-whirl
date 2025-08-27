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
use bevy::render::{
    renderer::{RenderDevice, RenderQueue},
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
};

// Simple GPU staging buffer state in the render world
#[derive(Resource)]
pub struct GpuCopyState { pub buffer: bevy::render::render_resource::Buffer, pub padded_bpr: usize, pub height: u32 }


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
    let needed_size = (padded_bpr as u64) * u64::from(cfg.height);
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
                bytes_per_row: Some(std::num::NonZeroU32::new(u32::try_from(state.padded_bpr).expect("padded_bpr fits in u32")).unwrap().into()),
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

// Mirror main-world handle into render-world so we can find the GPU image
pub fn extract_render_image_handle(
    mut commands: bevy::prelude::Commands,
    handle: Extract<Option<Res<RenderImageHandle>>>,
) {
    if let Some(h) = handle.as_deref() { commands.insert_resource(h.clone()); }
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
