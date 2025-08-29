
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
mod markers;
mod tuning;

#[derive(Clone, bevy::prelude::Resource)]
struct AllowExitFlag(std::sync::Arc<std::sync::atomic::AtomicBool>);

use crate::{
    ball::BallPlugin,
    capture::{ add_render_capture_systems, FrameSender },
    ffmpeg::{ spawn_ffmpeg, FfmpegHandle },
    setup::{ SetupPlugin, VideoExportRequest },
    tuning::{ spawn_axum_server, PhysicsTuning, TuningRx, TuningMirror },
};

fn main() {
    // Default: headless video recording with UDP preview
    // Optional: windowed/interactive via --windowed flag or WINDOWED=1 env var
    let windowed = std::env::args().any(|a| a == "--windowed")
        || std::env::var("WINDOWED").ok().is_some();

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgba(0.17, 0.18, 0.19, 1.0)));

    // Single source of truth for FPS
    let fps: u32 = std::env::var("VIDEO_FPS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);

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
        app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / fps as f64)));
        // Disable GPU preprocessing to avoid Core3D prepass requirement (headless)
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            use bevy::render::batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport};
            render_app.insert_resource(GpuPreprocessingSupport { max_supported_mode: GpuPreprocessingMode::None });
        }
    }

    // Initialize export pipeline by default in headless mode and hold ffmpeg handle for post-exit wait()
    let ff_handle: Option<FfmpegHandle> = if !windowed {
        // Provide export request; setup_graphics will create an offscreen target and camera
        app.insert_resource(VideoExportRequest { width: 1080, height: 1920, fps });

        // Frame channel to feed ffmpeg
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        app.insert_resource(FrameSender { tx });

        // Wire capture into render subapp
        add_render_capture_systems(&mut app);

        // Spawn ffmpeg thread
        Some(
            spawn_ffmpeg(1080, 1920, fps, rx)
                .expect("Failed to spawn ffmpeg; ensure it is installed and on PATH"),
        )
    } else {
        None
    };

    // Core scene plugins
    app.add_plugins(( SetupPlugin, BallPlugin ));

    // Install tuning HTTP server (Axum) and channel bridge
    use std::{net::SocketAddr, sync::{mpsc, Arc, Mutex}};
    let (tuning_tx, tuning_rx) = mpsc::channel();
    let tuning_mirror = Arc::new(Mutex::new(PhysicsTuning {
        rel_vel_min: 0.15,
        rel_vel_max: 360.0,
        break_force_threshold: 360.0,
        energy_transfer_enabled: true,
        energy_share_diff_threshold: 100,
        energy_share_friendly_rate: 0.5,
        energy_share_parent_not_friendly_child_friendly_rate: 0.75,
        energy_share_parent_friendly_child_not_friendly_rate: 0.25,
        energy_share_hostile_rand_min: 0.5,
        energy_share_hostile_rand_max: 0.9,
        bite_enabled: true,
        bite_size_scale: 1.0,
        show_collision_labels: true,
        collision_label_force_min: 2.0,
        show_break_labels: true,
        break_label_impulse_min: 20.0,
    }));
    app.insert_non_send_resource(TuningRx(tuning_rx));
    app.insert_resource(TuningMirror(tuning_mirror.clone()));
    spawn_axum_server(SocketAddr::from(([127,0,0,1], 7878)), tuning_tx, tuning_mirror);

    // System to apply updates from HTTP
    app.add_systems(Update, tuning::apply_tuning_updates_system);
    // Provide default tuning resource (so systems can read it)
    app.insert_resource(PhysicsTuning { rel_vel_min: 0.15, rel_vel_max: 360.0, break_force_threshold: 360.0, energy_transfer_enabled: true, energy_share_diff_threshold: 100, energy_share_friendly_rate: 0.5, energy_share_parent_not_friendly_child_friendly_rate: 0.75, energy_share_parent_friendly_child_not_friendly_rate: 0.25, energy_share_hostile_rand_min: 0.5, energy_share_hostile_rand_max: 0.9, bite_enabled: true, bite_size_scale: 1.0, show_collision_labels: true, collision_label_force_min: 2.0, show_break_labels: true, break_label_impulse_min: 20.0 });

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

