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
    let export_plugin = ImageExportPlugin::default();
    let export_threads = export_plugin.threads.clone();

    App::new()
    .add_plugins((
        DefaultPlugins,
        export_plugin,

        SetupPlugin,
        BallPlugin,

        // Adds a system that prints diagnostics to the console
        // LogDiagnosticsPlugin::default(),
        // Adds frame time diagnostics
        // FrameTimeDiagnosticsPlugin::default(),
        // Any plugin can register diagnostics. Uncomment this to add an entity count diagnostics:
        // bevy::diagnostic::EntityCountDiagnosticsPlugin::default(),
        // Uncomment this to add an asset count diagnostics:
        // bevy::asset::diagnostic::AssetCountDiagnosticsPlugin::<Texture>::default(),
        // Uncomment this to add system info diagnostics:
        // bevy::diagnostic::SystemInformationDiagnosticsPlugin::default()
    ))
    .run();

    // This line is optional but recommended.
    // It blocks the main thread until all image files have been saved successfully.
    export_threads.finish();
}