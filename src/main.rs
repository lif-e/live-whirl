use std::time::Duration;

use bevy::{
    color::Color,
    prelude::{
        App,
        Local,
        Time,
        Res,
        Update,
        Events,
        AppExit,
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

use crate::{
    ball::BallPlugin,
    capture::{
        add_render_capture_systems,
        FrameSender,
    },
    ffmpeg::{
        spawn_ffmpeg,
    },
    setup::{
        SetupPlugin,
        VideoExportRequest,
    },
};

fn main() {
    let want_render = std::env::var("RENDERING").ok().is_some();
    let render_stage: u32 = std::env::var("RENDER_STAGE").ok().and_then(|s| s.parse().ok()).unwrap_or(0);

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgba(0.17, 0.18, 0.19, 1.0)));


    if want_render {
        match render_stage {
            1 | 2 | 3 => {
                // Use DefaultPlugins configured for headless like the example
                use bevy::app::ScheduleRunnerPlugin;
                use bevy::winit::WinitPlugin;
                app.add_plugins(
                    bevy::DefaultPlugins
                        .set(ImagePlugin::default_nearest())
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
            _ => {
                // Full headless 2D stack
                app.add_plugins((
                    bevy::asset::AssetPlugin::default(),


                    ImagePlugin::default_nearest(),
                    bevy::render::RenderPlugin::default(),
                    bevy::core_pipeline::core_2d::Core2dPlugin,
                    bevy::sprite::SpritePlugin,
                    WindowPlugin { primary_window: None, ..Default::default() },

                ));
            }
        }
    }

    // Runner added at the end after all plugins/resources

    let video_export = std::env::var("VIDEO_EXPORT").ok().is_some();
    if want_render && render_stage >= 3 && video_export {
        // Provide export request; setup_graphics will create an offscreen target and camera
        app.insert_resource(VideoExportRequest { width: 1080, height: 1920, fps: 60 });

        // Frame channel to feed ffmpeg
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        app.insert_resource(FrameSender { tx });

        // Wire capture into render subapp
        add_render_capture_systems(&mut app);

        // Spawn ffmpeg thread
        let _ff = spawn_ffmpeg(1080, 1920, 60, rx)
            .expect("Failed to spawn ffmpeg; ensure it is installed and on PATH");
    }

    if want_render && render_stage >= 3 {
        app.add_plugins((
            SetupPlugin,
            BallPlugin,
        ));
    }


    // Simple heartbeat so we can see continuous progress
    app.add_systems(Update, heartbeat_log);
    // Prevent auto-exit when there are zero windows by clearing AppExit
    app.add_systems(bevy::app::Last, prevent_exit);


    app.run();
}

fn heartbeat_log(mut acc: Local<f32>, time: Res<Time>) {
    *acc += time.delta_secs();
    if *acc >= 1.0 {
        eprintln!("[diag] heartbeat");
        *acc = 0.0;
    }
}

fn prevent_exit(mut ev: ResMut<Events<AppExit>>) {
    ev.clear();
}