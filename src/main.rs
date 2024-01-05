use std::io::{Write, Cursor};
use std::process::{Command, Stdio};
use std::thread::{self, sleep};

use chrono::Duration;
use image::codecs::png::PngEncoder;
use image::{ImageBuffer, Rgb, ImageEncoder};
use rand::Rng;
use rand::{
    rngs::StdRng,
    SeedableRng,
};

use bevy::prelude::{
    App,
    DefaultPlugins,
};
use bevy_image_export::ImageExportPlugin;

mod shared_consts;
mod setup;
mod ball;
use crate::setup::SetupPlugin;
use crate::ball::BallPlugin;

fn main() {
    let mut rng = StdRng::seed_from_u64(42);
    // Set up the FFmpeg process    
    let handle = thread::spawn(move || {
        const CRF: u8 = 32;
        let now = chrono::Utc::now();
        let filename = format!("output_{}_{}.mp4", CRF, now.format("%Y-%m-%d_%H-%M-%S"));
        let mut child = match Command::new("ffmpeg")
            .args(&[
                "-f", "image2pipe",
                "-r", "60",
                "-i", "-",
                "-vcodec", "libx264",
                "-crf", &format!("{}", CRF),
                "-pix_fmt", "yuv420p",
                &filename,
            ])
            .stdin(Stdio::piped())
            .spawn() {
                Ok(child) => child,
                Err(e) => panic!("Failed to spawn FFmpeg process: {}", e),
            };
    
        // You can use child.wait() here if you want to wait for the process to finish
        // Get the stdin of the child process
        let mut stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => panic!("Failed to open stdin of FFmpeg process"),
        };
        // Write data to stdin
        
        for ndx in 0..600 {
            if rng.gen_bool(0.01) {
                sleep(std::time::Duration::from_millis(100));
            }
            let mut img = ImageBuffer::new(1020, 1920);
            for pixel in img.pixels_mut() {
                // let noise = rng.gen::<[u8; 3]>();
                *pixel = Rgb([
                    (((ndx % 60) as f32 / 60.0) * u8::MAX as f32) as u8,
                    rng.gen::<u8>(),
                    (((ndx % 60) as f32 / 60.0) * u8::MAX as f32) as u8,
                ]);
            }
            let mut buffer = Cursor::new(Vec::new());
            let _ = PngEncoder::new(&mut buffer).write_image(&img.into_raw(), 1020, 1920, image::ColorType::Rgb8);
            let _ = stdin.write_all(&buffer.into_inner());
        }
        let _ = stdin.flush();
        // std::mem::drop(stdin);
    });

    // You can use handle.join() to wait for the thread to finish
    let _ = handle.join();


    // let export_plugin = ImageExportPlugin::default();
    // let export_threads = export_plugin.threads.clone();

    // App::new()
    // .add_plugins((
    //     DefaultPlugins,
    //     export_plugin,

    //     SetupPlugin,
    //     BallPlugin,

    //     // Adds a system that prints diagnostics to the console
    //     // LogDiagnosticsPlugin::default(),
    //     // Adds frame time diagnostics
    //     // FrameTimeDiagnosticsPlugin::default(),
    //     // Any plugin can register diagnostics. Uncomment this to add an entity count diagnostics:
    //     // bevy::diagnostic::EntityCountDiagnosticsPlugin::default(),
    //     // Uncomment this to add an asset count diagnostics:
    //     // bevy::asset::diagnostic::AssetCountDiagnosticsPlugin::<Texture>::default(),
    //     // Uncomment this to add system info diagnostics:
    //     // bevy::diagnostic::SystemInformationDiagnosticsPlugin::default()
    // ))
    // .run();

    // // This line is optional but recommended.
    // // It blocks the main thread until all image files have been saved successfully.
    // export_threads.finish();
}