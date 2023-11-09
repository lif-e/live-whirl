use bevy::{
    prelude::{
        App,
        Assets,
        Camera,
        Camera2dBundle,
        Color,
        Commands,
        Mesh,
        OrthographicProjection,
        Plugin,
        ResMut,
        shape::Quad,
        Startup,
        Transform,
        Vec2,
    },
    core_pipeline::{
        bloom::{
            BloomCompositeMode,
            BloomPrefilterSettings,
            BloomSettings,
        },
        tonemapping::Tonemapping,
    },
    sprite::{
        ColorMaterial,
        MaterialMesh2dBundle,
    },
};
use bevy_rapier2d::prelude::{
    Collider,
    // DebugRenderMode,
    NoUserData,
    RapierConfiguration,
    // RapierDebugRenderPlugin,
    RapierPhysicsPlugin,
};

use crate::shared_consts::PIXELS_PER_METER;
use crate::ball::BALL_RADIUS;

pub fn setup_graphics(mut commands: Commands, mut rapier_config: ResMut<RapierConfiguration>) {
    rapier_config.gravity = Vec2::new(0.0, 0.0);

    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                hdr: true, // 1. HDR is required for bloom
                ..Camera::default()
            },
            tonemapping: Tonemapping::TonyMcMapface, // 2. Using an HDR tonemapper that desaturates to white is recommended
            projection: OrthographicProjection {
                near: -1000., // Camera2DBundle default that doesn't match OrthographicProjection default
                scale: 7.0,
                ..OrthographicProjection::default()
            },
            ..Camera2dBundle::default()
        },
        BloomSettings {
            intensity: 0.27848008 / 4.0,
            low_frequency_boost: 0.35, // 0.5019195,
            low_frequency_boost_curvature: 0.91, // 1.0,
            high_pass_frequency: 1.0,
            composite_mode: BloomCompositeMode::EnergyConserving,
            prefilter_settings : BloomPrefilterSettings {
                threshold: 0.0,
                threshold_softness: 0.0,
            },
            ..BloomSettings::default()
        },
    ));
}

const WALL_HALFTHICKNESS: f32 = 0.1 * PIXELS_PER_METER;
pub const WALLS_HALFHEIGHT: f32 = 5.0 * PIXELS_PER_METER;
pub const GROUND_HALFWIDTH: f32 = 4.0 * PIXELS_PER_METER;
pub const GROUND_POSITION: f32 = -5.0 * PIXELS_PER_METER;

pub fn setup_whirl(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut add_wall = |
        size: Vec2,
        position: Vec2,
    | {
        commands
            .spawn(Collider::cuboid(size.x / 2.0, size.y / 2.0))
            .insert(MaterialMesh2dBundle {
                mesh: meshes.add(Quad::new(size).into()).into(),
                material: materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0))),
                transform: Transform::from_xyz(position.x, position.y, 0.0),
                ..MaterialMesh2dBundle::default()
            })
        ;
    };
    println!("Setting up whirl");
    add_wall(
        Vec2::new(GROUND_HALFWIDTH * 2.0, WALL_HALFTHICKNESS * 2.0),
        Vec2::new(0.0, GROUND_POSITION),
    );
    add_wall(
        Vec2::new(WALL_HALFTHICKNESS * 2.0, WALLS_HALFHEIGHT * 2.0),
        Vec2::new(-1.0 * GROUND_HALFWIDTH, WALLS_HALFHEIGHT + GROUND_POSITION),
    );
    add_wall(
        Vec2::new(WALL_HALFTHICKNESS * 2.0, WALLS_HALFHEIGHT * 2.0),
        Vec2::new(GROUND_HALFWIDTH, WALLS_HALFHEIGHT + GROUND_POSITION),
    );
    add_wall(
        Vec2::new(GROUND_HALFWIDTH * 2.0, WALL_HALFTHICKNESS * 2.0),
        Vec2::new(0.0, 2.0 * WALLS_HALFHEIGHT + GROUND_POSITION),
    );

    const HORIZONTAL_SPACING: f32 = 0.5 * PIXELS_PER_METER;
    const VERTICAL_SPACING: f32 = 0.5 * PIXELS_PER_METER;
    const SPACED_WIDTH: i32 = ((GROUND_HALFWIDTH * 2.0) / HORIZONTAL_SPACING) as i32;
    const SPACED_HEIGHT: i32 = ((WALLS_HALFHEIGHT * 2.0) / VERTICAL_SPACING) as i32;

    for i in 0..SPACED_WIDTH {
        let x = (i as f32 * HORIZONTAL_SPACING) - GROUND_HALFWIDTH;
        for j in 1..SPACED_HEIGHT {
            let y = j as f32 * VERTICAL_SPACING + GROUND_POSITION;
            let row_shift: f32 = if j % 2 == 1 { 0.0 } else { HORIZONTAL_SPACING / 2.0 };
            if i == 0 && row_shift == 0.0 { continue; }
            add_wall(
                Vec2::new(BALL_RADIUS, BALL_RADIUS),
                Vec2::new(x + row_shift, y),
            );
        }
    }
}

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins((
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(PIXELS_PER_METER),
            // RapierDebugRenderPlugin {
            //     mode: (
            //         DebugRenderMode::COLLIDER_SHAPES
            //         // | DebugRenderMode::RIGID_BODY_AXES
            //         // | DebugRenderMode::MULTIBODY_JOINTS
            //         // | DebugRenderMode::IMPULSE_JOINTS
            //         // | DebugRenderMode::JOINTS
            //         // | DebugRenderMode::COLLIDER_AABBS
            //         // | DebugRenderMode::SOLVER_CONTACTS
            //         // | DebugRenderMode::CONTACTS
            //     ),
            //     ..RapierDebugRenderPlugin::default()
            // },
        ))
        .add_systems(
            Startup,
            (
                setup_graphics,
                setup_whirl,
            ),
        )
        ;
    }
}