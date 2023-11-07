use bevy::prelude::App;
use bevy::prelude::*;

mod shared_consts;
mod setup;
mod ball_source;
use crate::setup::SetupPlugin;
use crate::ball_source::BallSourcePlugin;

fn main() {
    App::new()
    .add_plugins((
        DefaultPlugins,

        SetupPlugin,
        BallSourcePlugin,
    ))
    .run();
}