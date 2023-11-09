use bevy::prelude::{
    App,
    DefaultPlugins,
};

mod shared_consts;
mod setup;
mod ball;
use crate::setup::SetupPlugin;
use crate::ball::BallPlugin;

fn main() {
    App::new()
    .add_plugins((
        DefaultPlugins,

        SetupPlugin,
        BallPlugin,
    ))
    .run();
}