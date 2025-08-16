use bevy::prelude::*;
use bevy::render::{
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_asset::RenderAssets,
    render_resource::{
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, ImageCopyBuffer,
        ImageCopyTexture, ImageDataLayout, MapMode, Origin3d, TextureAspect,
    },
    renderer::{RenderDevice, RenderQueue},
    texture::GpuImage,
    RenderApp,
};
use std::sync::mpsc::Sender;
use std::time::Duration;
use wgpu::{Maintain, COPY_BYTES_PER_ROW_ALIGNMENT};

#[derive(Resource, Clone, ExtractResource)]
pub struct FrameSender {
    pub tx: Sender<Vec<u8>>,
}

#[derive(Resource, Clone, ExtractResource)]
pub struct OffscreenTargetRender {
    pub handle: Handle<Image>,
    pub width: u32,
    pub height: u32,
}

#[derive(Resource, Default)]
struct ReadbackScratch {
    // Placeholder for future reuse; currently not reusing buffers
}

pub fn add_render_capture_systems(app: &mut App) {
    // Ensure these resources are extracted into the render world each frame
    app.add_plugins(ExtractResourcePlugin::<FrameSender>::default());
    app.add_plugins(ExtractResourcePlugin::<OffscreenTargetRender>::default());

    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return; };
    render_app
        .init_resource::<ReadbackScratch>()
        .add_systems(bevy::render::Render, capture_and_send_frame);
}

fn capture_and_send_frame(
    images: Res<RenderAssets<GpuImage>>,
    target: Option<Res<OffscreenTargetRender>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    sender: Option<Res<FrameSender>>,
) {
    let (target, sender) = match (target, sender) {
        (Some(t), Some(s)) => (t, s),
        _ => return,
    };

    let gpu_image: &GpuImage = match images.get(&target.handle) {
        Some(img) => img,
        None => return,
    };

    // Ensure expected format
    let width = target.width;
    let height = target.height;
    let bytes_per_pixel = 4u32; // RGBA8
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u32; // 256
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    // Create readback buffer
    let buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("frame_readback_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Record copy command
    let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("frame_readback_encoder"),
    });

    encoder.copy_texture_to_buffer(
        ImageCopyTexture {
            texture: &gpu_image.texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        ImageCopyBuffer {
            buffer: &buffer,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        Extent3d { width, height, depth_or_array_layers: 1 },
    );

    render_queue.submit(std::iter::once(encoder.finish()));

    // Map and wait for readiness (allow some time)
    let slice = buffer.slice(..);
    let (tx_ready, rx_ready) = std::sync::mpsc::channel::<bool>();
    slice.map_async(MapMode::Read, move |res| {
        let _ = tx_ready.send(res.is_ok());
    });

    // Poll the GPU; Bevy will also poll each frame, but we give it a moment now
    render_device.poll(wgpu::Maintain::Wait);

    let Ok(true) = rx_ready.recv_timeout(Duration::from_millis(2000)) else { return; };

    let data = slice.get_mapped_range();
    let mut rgba = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        rgba.extend_from_slice(&data[start..end]);
    }
    drop(data);
    buffer.unmap();

    let _ = sender.tx.send(rgba);
}

