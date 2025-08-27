
use std::time::Duration;

use bevy::{
    app::{
        Last,
    },
    color::Color,
    prelude::{
        App,
        Update,
        Events,
        AppExit,
        Res,
        ResMut,
        PluginGroup,
    },
    render::{
        camera::ClearColor,
        texture::ImagePlugin,
    },
    window::WindowPlugin,
};




mod ball;
mod capture;
mod ffmpeg;
mod setup;
mod shared_consts;

#[derive(Clone, bevy::prelude::Resource)]
struct AllowExitFlag(std::sync::Arc<std::sync::atomic::AtomicBool>);

use crate::{
    ball::BallPlugin,
    capture::{
        add_render_capture_systems,
        FrameSender,
    },
    ffmpeg::{
        spawn_ffmpeg,
        FfmpegHandle,
    },
    setup::{
        SetupPlugin,
        VideoExportRequest,
    },
};

fn main() {
    // Default: headless video recording with UDP preview
    // Optional: windowed/interactive via --windowed flag or WINDOWED=1 env var
    let windowed = std::env::args().any(|a| a == "--windowed")
        || std::env::var("WINDOWED").ok().is_some();

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgba(0.17, 0.18, 0.19, 1.0)));

    if windowed {
        // Standard windowed stack
        app.add_plugins(
            bevy::DefaultPlugins
                .set(ImagePlugin::default_nearest())
        );
    } else {
        // Headless stack modeled after Bevy headless example
        use bevy::app::ScheduleRunnerPlugin;
        use bevy::winit::WinitPlugin;
        app.add_plugins(
            bevy::DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(bevy::log::LogPlugin { level: bevy::log::Level::INFO, filter: "bevy_window::system=error".into(), ..Default::default() })
                .set(WindowPlugin { primary_window: None, ..Default::default() })
                .disable::<WinitPlugin>(),
        );
        app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)));
        // Disable GPU preprocessing to avoid Core3D prepass requirement (headless)
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            use bevy::render::batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport};
            render_app.insert_resource(GpuPreprocessingSupport { max_supported_mode: GpuPreprocessingMode::None });
        }
    }

    // Initialize export pipeline by default in headless mode and hold ffmpeg handle for post-exit wait()
    let ff_handle: Option<FfmpegHandle> = if !windowed {
        // Provide export request; setup_graphics will create an offscreen target and camera
        app.insert_resource(VideoExportRequest { width: 1080, height: 1920, fps: 60 });

        // Frame channel to feed ffmpeg
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        app.insert_resource(FrameSender { tx });

        // Wire capture into render subapp
        add_render_capture_systems(&mut app);

        // Spawn ffmpeg thread
        Some(
            spawn_ffmpeg(1080, 1920, 60, rx)
                .expect("Failed to spawn ffmpeg; ensure it is installed and on PATH"),
        )
    } else {
        None
    };

    // Core scene plugins
    app.add_plugins((
        SetupPlugin,
        BallPlugin,
    ));

    if !windowed {
        // Prevent auto-exit when there are zero windows by clearing AppExit (gated by exit flag)
        app.add_systems(Last, prevent_exit);

        // Install Ctrl+C and stdin-EOF shutdown triggers
        let exit_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.insert_resource(AllowExitFlag(exit_flag.clone()));
        {
            let f2 = exit_flag.clone();
            let _ = ctrlc::set_handler(move || {
                eprintln!("[diag] SIGINT received, requesting shutdown...");
                f2.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        {
            let f3 = exit_flag; // move into thread without redundant clone
            std::thread::spawn(move || {
                use std::io::Read;
                let mut stdin = std::io::stdin();
                let mut buf = [0u8; 1];
                loop {
                    match stdin.read(&mut buf) {
                        Ok(0) => { // EOF
                            eprintln!("[diag] stdin EOF; requesting shutdown...");
                            f3.store(true, std::sync::atomic::Ordering::SeqCst);
                            break;
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            });
        }
        // Per-frame: send AppExit when flag is set
        app.add_systems(Update, |flag: Res<AllowExitFlag>, mut ev: ResMut<Events<AppExit>>| {
            if flag.0.load(std::sync::atomic::Ordering::SeqCst) {
                ev.clear();
                ev.send(AppExit::Success);
            }
        });
    }

    app.run();

    // After app exits, wait on ffmpeg so the MP4 finalizes cleanly.
    if let Some(mut h) = ff_handle {
        let _ = h.child.wait();
    }




fn prevent_exit(flag: Option<Res<AllowExitFlag>>, mut ev: ResMut<Events<AppExit>>) {
    if let Some(f) = flag {
        if f.0.load(std::sync::atomic::Ordering::SeqCst) {
            // shutdown requested; do not suppress exit
            return;
        }
    }
    // otherwise, suppress auto-exit when no windows are open
    ev.clear();
}

}

