// use std::io::{Write, Cursor};
// use std::process::{Command, Stdio};
// use std::thread::{self, sleep};

// use chrono::Duration;
// use image::codecs::png::PngEncoder;
// use image::{ImageBuffer, Rgb, ImageEncoder};
// use rand::Rng;
// use rand::{
//     rngs::StdRng,
//     SeedableRng,
// };

use bevy::{
    prelude::{App, DefaultPlugins, PluginGroup},
};
use bevy::app::ScheduleRunnerPlugin;
use bevy::winit::WinitPlugin;
use std::time::Duration;
// use bevy_image_export::ImageExportPlugin;

mod shared_consts;
mod setup;
mod capture;
mod ffmpeg;
mod ball;
mod camera_tuning;
mod ball_sync;

use crate::setup::SetupPlugin;
use crate::ball::BallPlugin;

fn main() {
    // let mut rng = StdRng::seed_from_u64(42);
    // // Set up the FFmpeg process
    // let handle = thread::spawn(move || {
    //     const CRF: u8 = 32;
    //     let now = chrono::Utc::now();
    //     let filename = format!("output_{}_{}.mp4", CRF, now.format("%Y-%m-%d_%H-%M-%S"));
    //     let mut child = match Command::new("ffmpeg")
    //         .args(&[
    //             "-f", "image2pipe",
    //             "-r", "60",
    //             "-i", "-",
    //             "-vcodec", "libx264",
    //             "-crf", &format!("{}", CRF),
    //             "-pix_fmt", "yuv420p",
    //             &filename,
    //         ])
    //         .stdin(Stdio::piped())
    //         .spawn() {
    //             Ok(child) => child,
    //             Err(e) => panic!("Failed to spawn FFmpeg process: {}", e),
    //         };

    //     // You can use child.wait() here if you want to wait for the process to finish
    //     // Get the stdin of the child process
    //     let mut stdin = match child.stdin.take() {
    //         Some(stdin) => stdin,
    //         None => panic!("Failed to open stdin of FFmpeg process"),
    //     };
    //     // Write data to stdin

    //     for ndx in 0..600 {
    //         if rng.gen_bool(0.01) {
    //             sleep(std::time::Duration::from_millis(100));
    //         }
    //         let mut img = ImageBuffer::new(1020, 1920);
    //         for pixel in img.pixels_mut() {
    //             // let noise = rng.gen::<[u8; 3]>();
    //             *pixel = Rgb([
    //                 (((ndx % 60) as f32 / 60.0) * u8::MAX as f32) as u8,
    //                 rng.gen::<u8>(),
    //                 (((ndx % 60) as f32 / 60.0) * u8::MAX as f32) as u8,
    //             ]);
    //         }
    //         let mut buffer = Cursor::new(Vec::new());
    //         let _ = PngEncoder::new(&mut buffer).write_image(&img.into_raw(), 1020, 1920, image::ColorType::Rgb8);
    //         let _ = stdin.write_all(&buffer.into_inner());
    //     }
    //     let _ = stdin.flush();
    //     // std::mem::drop(stdin);
    // });

    // // You can use handle.join() to wait for the thread to finish
    // let _ = handle.join();


    let video_export = std::env::var("VIDEO_EXPORT").ok().is_some();

    // let export_plugin = ImageExportPlugin::default();
    // let export_threads = export_plugin.threads.clone();

    let headless = std::env::var("HEADLESS").ok().is_some();

    let mut app = App::new();
    if headless {
        app.insert_resource(crate::setup::Headless(true));
        let debug_window_driver = std::env::var("DEBUG_WINDOW_DRIVER").ok().is_some();
        // For video export, always create a tiny hidden window to ensure RenderDevice exists
        let use_hidden_window = debug_window_driver || video_export;
        if use_hidden_window {
            // Tiny hidden window to drive the render loop headlessly (offscreen capture requires a RenderDevice)
            let plugins = DefaultPlugins
                .set(bevy::window::WindowPlugin {
                    primary_window: Some(bevy::window::Window {
                        resolution: (1., 1.).into(),
                        visible: false,
                        present_mode: bevy::window::PresentMode::AutoNoVsync,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .disable::<bevy::pbr::PbrPlugin>()
                .disable::<bevy::gltf::GltfPlugin>();
            app.add_plugins(plugins);
        } else {
            // Pure headless ECS loop; no rendering
            app.add_plugins(
                DefaultPlugins
                    .set(bevy::window::WindowPlugin { primary_window: None, ..Default::default() })
                    .disable::<WinitPlugin>()
                    .disable::<bevy::gltf::GltfPlugin>()
            );
            app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0/60.0)));
        }
    } else {
        use bevy::window::{Window, WindowPlugin, PresentMode};
        // Make the window manageable on small displays and stable for viewing
        let plugins = DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (540., 960.).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..Default::default()
            }),
            ..Default::default()
        });
        app.add_plugins(plugins);
    }
    // Material2dPlugin<ColorMaterial> is already included via DefaultPlugins in Bevy 0.16.

    // If video export is requested, configure offscreen target and capture
    if video_export {
        // Provide export request; setup_graphics will create an offscreen target and camera
        app.insert_resource(crate::setup::VideoExportRequest { width: 1080, height: 1920, fps: 60 });

        // Frame channel to feed ffmpeg
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        app.insert_resource(crate::capture::FrameSender { tx });

        // Wire capture into render subapp
        crate::capture::add_render_capture_systems(&mut app);

        // Spawn ffmpeg thread
        let _ff = crate::ffmpeg::spawn_ffmpeg(1080, 1920, 60, rx)
            .expect("Failed to spawn ffmpeg; ensure it is installed and on PATH");
    }

    if headless {
        // Only use a manual runner when we truly have no window to drive the render loop
        let debug_window_driver = std::env::var("DEBUG_WINDOW_DRIVER").ok().is_some();
        if !(debug_window_driver || video_export) {
            app.set_runner(|mut app| {
                use std::{thread, time::Duration};
                loop {
                    app.update();
                    thread::sleep(Duration::from_millis(5));
                }
            });
        }
    }

    app.add_plugins((
        SetupPlugin,
        BallPlugin,
    ))
    .run();

    // // This line is optional but recommended.
    // // It blocks the main thread until all image files have been saved successfully.
    // export_threads.finish();
}