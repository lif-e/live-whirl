use bevy::{
    prelude::{
        App,
        Assets,
        Camera,
        Camera2dBundle,
        Color,
        Commands,
        Component,
        default,
        Mesh,
        OrthographicProjection,
        Plugin,
        ResMut,
        Resource,
        shape::{
            Quad,
            Box,
        },
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
    render::{
        render_resource::{
            Extent3d,
            TextureDescriptor,
            TextureDimension,
            TextureFormat,
            TextureUsages,
        },
        texture,
        camera::RenderTarget,
    },
};
use bevy_rapier2d::{
    prelude::{
        Collider,
        // DebugRenderMode,
        NoUserData,
        RapierConfiguration,
        // RapierDebugRenderPlugin,
        RapierPhysicsPlugin,
    },
    plugin::TimestepMode,
};
use bevy_image_export::{
    ImageExportBundle,
    ImageExportSource,
    ImageExportSettings,
};
use rand::{
    rngs::StdRng,
    SeedableRng,
};

use crate::shared_consts::PIXELS_PER_METER;
use crate::ball::BALL_RADIUS;

#[derive(Resource)]
pub struct RngResource {
    pub rng: StdRng,
}

pub fn setup_graphics(
    mut commands: Commands,
    mut rapier_config: ResMut<RapierConfiguration>,
    mut images: ResMut<Assets<texture::Image>>,
    mut export_sources: ResMut<Assets<ImageExportSource>>,
) {

    // Create an output texture.
    let output_texture_handle = {
        let size = Extent3d {
            width: 1080,
            height: 1920,
            ..default()
        };
        let mut export_texture = texture::Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
            ..default()
        };
        export_texture.resize(size);

        images.add(export_texture)
    };
    
    // rapier_config.gravity = Vec2::new(0.0, -9.8 * PIXELS_PER_METER * 0.000625);
    rapier_config.gravity = Vec2::new(0.0, -9.8 * PIXELS_PER_METER * 0.000625 * 100.0);
    rapier_config.timestep_mode = TimestepMode::Fixed {
        /// The physics simulation will be advanced by this total amount at each Bevy tick.
        dt: 1.0 / 60.0,
        /// This number of substeps of length `dt / substeps` will be performed at each Bevy tick.
        substeps: 1,
    };

    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                hdr: true, // 1. HDR is required for bloom
                target: RenderTarget::Image(output_texture_handle.clone()),
                ..Camera::default()
            },
            tonemapping: Tonemapping::TonyMcMapface, // 2. Using an HDR tonemapper that desaturates to white is recommended
            projection: OrthographicProjection {
                near: -1000., // Camera2DBundle default that doesn't match OrthographicProjection default
                // scale: 6.0,
                scale: 4.0,
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

    commands.spawn(ImageExportBundle {
        source: export_sources.add(output_texture_handle.into()),
        settings: ImageExportSettings {
            // Frames will be saved to "./out/[#####].png".
            output_dir: "out".into(),
            // Choose "exr" for HDR renders.
            extension: "png".into(),
        },
    });
}

pub const WALL_HEIGHT: f32 = 9.0 * PIXELS_PER_METER * 1.62068966;
pub const GROUND_WIDTH: f32 = 8.0 * PIXELS_PER_METER;
pub const WALL_THICKNESS: f32 = 0.1 * PIXELS_PER_METER;

const GROUND_POSITION: f32 = -0.5 * WALL_HEIGHT;

const WALL_BOX: Box = Box {
    min_x: -0.5 * GROUND_WIDTH,
    max_x:  0.5 * GROUND_WIDTH,
    min_y: GROUND_POSITION,
    max_y: WALL_HEIGHT + GROUND_POSITION,
    min_z: 0.0,
    max_z: 0.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct Wall;

impl Default for Wall {
    fn default() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct Peg;

impl Default for Peg {
    fn default() -> Self {
        Self
    }
}

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
            .spawn((
                Wall::default(),
                Collider::cuboid(size.x / 2.0, size.y / 2.0),
                MaterialMesh2dBundle {
                    mesh: meshes.add(Quad::new(size).into()).into(),
                    material: materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0))),
                    transform: Transform::from_xyz(position.x, position.y, 0.0),
                    ..MaterialMesh2dBundle::default()
                },
            ));
    };
    println!("Setting up whirl");
    const VERTICAL_WALLS_SIZE: Vec2 = Vec2::new(WALL_BOX.max_x - WALL_BOX.min_x, WALL_THICKNESS);
    const HORIZONTAL_WALLS_SIZE: Vec2 = Vec2::new(WALL_THICKNESS, WALL_BOX.max_y - WALL_BOX.min_y);
    add_wall(
        VERTICAL_WALLS_SIZE,
        Vec2 { x: 0.0, y: WALL_BOX.min_y },
    );
    add_wall(
        VERTICAL_WALLS_SIZE,
        Vec2::new(0.0, WALL_BOX.max_y),
    );
    add_wall(
        HORIZONTAL_WALLS_SIZE,
        Vec2::new(WALL_BOX.min_x, 0.0),
    );
    add_wall(
        HORIZONTAL_WALLS_SIZE,
        Vec2::new(WALL_BOX.max_x, 0.0),
    );

    const HORIZONTAL_SPACING: f32 = 0.5 * PIXELS_PER_METER;
    const VERTICAL_SPACING: f32 = 0.5 * PIXELS_PER_METER;
    const SPACED_WIDTH: i32 = (GROUND_WIDTH / HORIZONTAL_SPACING) as i32;
    const SPACED_HEIGHT: i32 = (WALL_HEIGHT / VERTICAL_SPACING) as i32;

    for i in 0..SPACED_WIDTH {
        let x = (i as f32 * HORIZONTAL_SPACING) - (0.5 * GROUND_WIDTH);
        for j in 1..SPACED_HEIGHT {
            let y = j as f32 * VERTICAL_SPACING + GROUND_POSITION;
            let row_shift: f32 = if j % 2 == 1 { 0.0 } else { HORIZONTAL_SPACING / 2.0 };
            if i == 0 && row_shift == 0.0 { continue; }
            let size = Vec2::new(BALL_RADIUS, BALL_RADIUS);
            let position = Vec2::new(x + row_shift, y);
            commands
                .spawn((
                    Peg::default(),
                    Collider::cuboid(size.x / 2.0, size.y / 2.0),
                    MaterialMesh2dBundle {
                        mesh: meshes.add(Quad::new(size).into()).into(),
                        material: materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0))),
                        transform: Transform::from_xyz(position.x, position.y, 0.0),
                        ..MaterialMesh2dBundle::default()
                    },
                ));
        }
    }
}

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app
        .insert_resource(RngResource {
            rng: StdRng::seed_from_u64(42),
        })
        .add_plugins((
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(PIXELS_PER_METER),
            // RapierDebugRenderPlugin {
            //     mode: (
            //         DebugRenderMode::COLLIDER_SHAPES
            //         // | DebugRenderMode::RIGID_BODY_AXES
            //         // | DebugRenderMode::MULTIBODY_JOINTS
            //         | DebugRenderMode::IMPULSE_JOINTS
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