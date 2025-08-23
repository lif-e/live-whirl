use bevy::prelude::*;
use bevy::render::{
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_asset::RenderAssets,
    render_resource::{
        Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d,
        MapMode, Origin3d, TextureAspect,
    },
    renderer::{RenderDevice, RenderQueue},
    texture::GpuImage,
    RenderApp,
};

use std::sync::{Arc, atomic::AtomicBool};
use std::sync::mpsc::Sender;

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

struct PendingReadback {
    buffer: Buffer,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
    unpadded_bytes_per_row: u32,
    ready: Arc<AtomicBool>,
    frame_ndx: u64,
}

#[derive(Resource, Default)]
struct ReadbackScratch {
    pending: Option<PendingReadback>,
    frame_index: u64,
    dumps_done: u32,
}

pub fn add_render_capture_systems(app: &mut App) {
    // Ensure these resources are extracted into the render world each frame
    app.add_plugins(ExtractResourcePlugin::<FrameSender>::default());
    app.add_plugins(ExtractResourcePlugin::<OffscreenTargetRender>::default());

    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };


    render_app
        .init_resource::<ReadbackScratch>()
        // Capture after the offscreen camera has rendered into the image
        .add_systems(
            bevy::render::Render,
            capture_and_send_frame.in_set(bevy::render::RenderSet::Cleanup),
        );
}

fn capture_and_send_frame(
    images: Res<RenderAssets<GpuImage>>,
    target: Option<Res<OffscreenTargetRender>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut scratch: ResMut<ReadbackScratch>,
    sender: Option<Res<FrameSender>>,
) {
    let (target, sender) = match (target, sender) {
        (Some(t), Some(s)) => (t, s),
        _ => return,
    };

    // If a readback is pending, check if it's ready and finish it
    if let Some(pending) = scratch.pending.take() {
        if pending.ready.load(std::sync::atomic::Ordering::Acquire) {
            let slice = pending.buffer.slice(..);
            let data = slice.get_mapped_range();
            let mut rgba = Vec::with_capacity((pending.width * pending.height * 4) as usize);
            for row in 0..pending.height {
                let start = (row * pending.padded_bytes_per_row) as usize;
                let end = start + pending.unpadded_bytes_per_row as usize;
                rgba.extend_from_slice(&data[start..end]);
            }
            drop(data);
            pending.buffer.unmap();

            // Write periodic dumps: 600, 1200, 1800, 2400 (at most 4)
            match pending.frame_ndx {
                600 | 1200 | 1800 | 2400 => {
                    let path = format!("frame_{:04}.rgba", pending.frame_ndx);
                    if std::fs::write(&path, &rgba).is_ok() {
                        eprintln!("[diag] wrote {} ({} bytes)", path, rgba.len());
                    }
                }
                0 => {
                    // One-shot first frame dump
                    let _ = std::fs::write("first_frame.rgba", &rgba);
                    eprintln!("[diag] wrote first_frame.rgba ({} bytes)", rgba.len());
                }
                _ => {}
            }

            // Lightweight content checksum to help detect duplicate frames
            // Full-frame checksum to detect frame-to-frame changes
            let checksum: u64 = rgba.chunks_exact(8)
                .map(|b| u64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]]))
                .fold(0u64, |acc, v| acc.wrapping_mul(1315423911).wrapping_add(v));
            eprintln!("[diag] frame {} checksum {:016x}", pending.frame_ndx, checksum);

            // Also log the first and last 16 bytes for quick eyeballing of changes
            if pending.frame_ndx % 120 == 0 {
                if rgba.len() >= 32 {
                    eprintln!("[diag] frame {} head {:02x?} tail {:02x?}", pending.frame_ndx, &rgba[..16], &rgba[rgba.len()-16..]);
                }
            }

            let _ = sender.tx.send(rgba);
        } else {
            // Not ready yet; put it back and try again next frame
            scratch.pending = Some(pending);
            return;
        }
    }

    // Start a new readback for the current frame
    let gpu_image: &GpuImage = match images.get(&target.handle) {
        Some(img) => img,
        None => return,
    };

    scratch.frame_index = scratch.frame_index.wrapping_add(1);

    let width = target.width;
    let height = target.height;
    let bytes_per_pixel = 4u32; // RGBA8
    let align = 256u32; // COPY_BYTES_PER_ROW_ALIGNMENT
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    let buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("frame_readback_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("frame_readback_encoder"),
    });

    use std::num::NonZeroU32;
    encoder.copy_texture_to_buffer(
        bevy::render::render_resource::TexelCopyTextureInfo {
            texture: &gpu_image.texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bevy::render::render_resource::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: bevy::render::render_resource::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(NonZeroU32::new(padded_bytes_per_row).unwrap().into()),
                rows_per_image: Some(NonZeroU32::new(height).unwrap().into()),
            },
        },
        Extent3d { width, height, depth_or_array_layers: 1 },
    );

    render_queue.submit(std::iter::once(encoder.finish()));

    let slice = buffer.slice(..);
    let ready = Arc::new(AtomicBool::new(false));
    let ready_cb = ready.clone();
    let frame_ndx = scratch.frame_index;
    slice.map_async(MapMode::Read, move |res| {
        if res.is_ok() {
            ready_cb.store(true, std::sync::atomic::Ordering::Release);
        }
        eprintln!("[diag] capture map_async for frame {} => {:?}", frame_ndx, res);
    });

    // Store pending readback; Bevy will drive GPU progress between frames
    scratch.pending = Some(PendingReadback {
        buffer,
        width,
        height,
        padded_bytes_per_row,
        unpadded_bytes_per_row,
        ready,
        frame_ndx,
    });
}

// (disabled) Option A diagnostic pass was here; removed to avoid altering the image.

