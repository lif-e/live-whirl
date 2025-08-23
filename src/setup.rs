use bevy::{

    prelude::{
        App, Assets, Camera, Camera2d, Color, Commands, Component, Handle, Image, Mesh,
        Plugin, Query, Res, ResMut, Resource, Startup, Update, Transform, Local, Vec2, With, PostUpdate, Time,
    },
    render::{
        prelude::Mesh2d,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::RenderLayers,
    },
    sprite::{ColorMaterial, MeshMaterial2d},
};
use bevy_rapier2d::{
    plugin::TimestepMode,
    prelude::{
        Collider,
        // DebugRenderMode,
        NoUserData,
        RapierConfiguration,
        // RapierDebugRenderPlugin,
        RapierPhysicsPlugin,
    },
};
// use bevy_image_export::{
//     ImageExportBundle,

//     ImageExportSource,
//     ImageExportSettings,
// };
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::ball::BALL_RADIUS;
use crate::shared_consts::PIXELS_PER_METER;

#[derive(Resource)]
pub struct DebugOverlay(pub bool);

#[derive(Component)]
struct OffscreenOverlay;

#[derive(Resource)]
pub struct RngResource {
    pub rng: StdRng,
}

#[derive(Resource, Default)]
pub struct MeshAssets2d {
    pub ball_circle: Handle<Mesh>,
}

#[derive(Resource)]
pub struct Headless(pub bool);

pub fn setup_meshes(mut meshes: ResMut<Assets<Mesh>>, mut commands: Commands) {
    let circle = meshes.add(bevy::math::primitives::Circle::new(
        super::ball::BALL_RADIUS,
    ));
    commands.insert_resource(MeshAssets2d {
        ball_circle: circle,
    });
}

#[derive(Resource, Default, Clone, Copy)]
pub struct VideoExportRequest {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

use crate::camera_tuning::OffscreenCam;

pub fn setup_graphics(
    mut commands: Commands,
    mut rapier_config_q: Query<&mut RapierConfiguration, With<bevy_rapier2d::plugin::context::DefaultRapierContext>>,
    mut timestep_mode: ResMut<TimestepMode>,
    headless: Option<Res<Headless>>,
    mut images: ResMut<Assets<Image>>,
    video_req: Option<Res<VideoExportRequest>>,
) {
    let has_video = video_req.is_some();
    let export = video_req.as_deref().copied().unwrap_or(VideoExportRequest {
        width: 1080,
        height: 1920,
        fps: 60,
    });

    let mut offscreen_handle_opt: Option<Handle<Image>> = None;
    if has_video {
        let mut offscreen_image = Image::new_fill(
            Extent3d {
                width: export.width,
                height: export.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0u8; 4],
            TextureFormat::Rgba8UnormSrgb,
            bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
        );
        offscreen_image.texture_descriptor.usage |=
            TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
        let offscreen_handle = images.add(offscreen_image);
        commands.insert_resource(crate::capture::OffscreenTargetRender {
            handle: offscreen_handle.clone(),
            width: export.width,
            height: export.height,
        });
        offscreen_handle_opt = Some(offscreen_handle);
    }

    let is_headless = matches!(headless.as_deref(), Some(Headless(true)));

    // // Create an output texture.
    // let output_texture_handle = {
    //     let size = Extent3d {
    //         width: 1080,
    //         height: 1920,
    //         ..default()
    //     };
    //     let mut export_texture = texture::Image {
    //         texture_descriptor: TextureDescriptor {
    //             label: None,
    //             size,
    //             dimension: TextureDimension::D2,
    //             format: TextureFormat::Rgba8UnormSrgb,
    //             mip_level_count: 1,
    //             sample_count: 1,
    //             usage: TextureUsages::COPY_DST
    //                 | TextureUsages::COPY_SRC
    //                 | TextureUsages::RENDER_ATTACHMENT,
    //             view_formats: &[],
    //         },
    //         ..default()
    //     };
    //     export_texture.resize(size);

    //     images.add(export_texture)
    // };

    if let Ok(mut rc) = rapier_config_q.single_mut() {
        rc.gravity = Vec2::new(0.0, -9.8 * PIXELS_PER_METER * 0.000625 * 100.0);
    }
    *timestep_mode = TimestepMode::Fixed {
        dt: 1.0 / 60.0,
        substeps: 1,
    };

    if !is_headless {
        use bevy::render::camera::ClearColorConfig;
        // Spawn a windowed camera that matches the offscreen framing and layers
        let scale_x = GROUND_WIDTH / export.width as f32;
        let scale_y = WALL_HEIGHT / export.height as f32;
        let fit_scale = scale_x.max(scale_y);
        let center_x = 0.5 * (WALL_BOX.min_x + WALL_BOX.max_x);
        let center_y = 0.5 * (WALL_BOX.min_y + WALL_BOX.max_y);
        let mut cam = Camera { hdr: false, ..Default::default() };
        cam.clear_color = ClearColorConfig::Custom(Color::srgba(0.17, 0.18, 0.19, 1.0));
        let win_cam = commands.spawn((
            Camera2d,
            cam,
            Transform::from_xyz(center_x, center_y, 1000.0),
            RenderLayers::from_layers(&[0, 1]),
        )).id();
        // Match the zoom used for offscreen (slightly beyond fit)
        use bevy::render::camera::{Projection, OrthographicProjection};
        let mut ortho = OrthographicProjection::default_2d();
        ortho.scale = fit_scale * 1.1;
        commands.entity(win_cam).insert(Projection::Orthographic(ortho));
    }

    // Always include a minimal debug overlay marker we can draw if desired; enable in video mode
    commands.insert_resource(DebugOverlay(has_video));

    if let Some(offscreen_handle) = &offscreen_handle_opt {
        let is_headless = matches!(headless.as_deref(), Some(Headless(true)));

        let scale_x = GROUND_WIDTH / export.width as f32;
        let scale_y = WALL_HEIGHT / export.height as f32;
        let fit_scale = scale_x.max(scale_y);
        let center_x = 0.5 * (WALL_BOX.min_x + WALL_BOX.max_x);
        let center_y = 0.5 * (WALL_BOX.min_y + WALL_BOX.max_y);

        // Offscreen camera centered on playfield, with orthographic scale set to fit entire area
        use bevy::math::UVec2;
        use bevy::render::camera::Viewport;
        use bevy::render::camera::ClearColorConfig;
        let mut cam = Camera { hdr: false, target: bevy::render::camera::RenderTarget::Image(offscreen_handle.clone().into()), order: 1, ..Default::default() };
        cam.clear_color = ClearColorConfig::Custom(Color::srgba(0.17, 0.18, 0.19, 1.0));
        cam.viewport = Some(Viewport { physical_position: UVec2::new(0, 0), physical_size: UVec2::new(export.width, export.height), depth: 0.0..1.0 });
        let off_cam = commands.spawn((
            Camera2d,
            cam,
            Transform::from_xyz(center_x, center_y, 1000.0),
            RenderLayers::from_layers(&[0, 1]),
            OffscreenCam,
        )).id();
        // Explicitly set orthographic scale to match windowed fit
        {
            use bevy::render::camera::{Projection, OrthographicProjection};
            let mut ortho = OrthographicProjection::default_2d();
            ortho.scale = fit_scale * 1.1;
            commands.entity(off_cam).insert(Projection::Orthographic(ortho));
        }

        // Force 5 debug sprites at the exact offscreen center in headless+video
        if is_headless && has_video {
            use bevy::sprite::Sprite;
            eprintln!("[diag] spawning 5 center debug sprites at ({center_x:.1},{center_y:.1})");
            for i in 0..5u32 {
                let dx = (i as f32 - 2.0) * 40.0;
                commands.spawn((
                    Sprite::from_color(Color::hsl(0.0 + 30.0 * i as f32, 1.0, 0.6), Vec2::new(48.0, 48.0)),
                    bevy::render::view::Visibility::Visible,
                    bevy::prelude::InheritedVisibility::VISIBLE,
                    Transform::from_xyz(center_x + dx, center_y, 40.0),
                    bevy::render::view::NoFrustumCulling,
                ));
            }
        }

        // Provide desired ortho scale so camera_tuning system applies it next frame
        commands.insert_resource(crate::camera_tuning::OrthoScale(fit_scale * 1.1));
    }
}

fn wiggle_offscreen_overlay(time: Res<Time>, mut q: Query<&mut Transform, With<OffscreenOverlay>>) {
    let t = time.elapsed_secs_wrapped();
    let dx = (t * 2.0).sin() * 10.0; // 10px left-right
    for mut tf in &mut q {
        tf.translation.x = tf.translation.x.signum() * tf.translation.x.abs().max(1.0) + dx;
    }
}

// Blink overlay removed (was causing strobe). Keeping code here for reference if needed:
// struct BlinkOverlay; ...

pub const WALL_HEIGHT: f32 = 9.0 * PIXELS_PER_METER * 1.62068966;
pub const GROUND_WIDTH: f32 = 8.0 * PIXELS_PER_METER;
pub const WALL_THICKNESS: f32 = 0.1 * PIXELS_PER_METER;

const GROUND_POSITION: f32 = -0.5 * WALL_HEIGHT;

struct Box2D {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    min_z: f32,
    max_z: f32,
}
const WALL_BOX: Box2D = Box2D {
    min_x: -0.5 * GROUND_WIDTH,
    max_x: 0.5 * GROUND_WIDTH,
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

// Spawn a single bright test ball in headless+video to validate offscreen rendering
pub(crate) fn spawn_headless_test_ball(
    mut commands: Commands,
    headless: Option<Res<Headless>>,
    video_req: Option<Res<VideoExportRequest>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mesh_assets: Option<Res<MeshAssets2d>>,
    mut spawned: Local<bool>,
) {
    if *spawned { return; }
    let is_headless = matches!(headless.as_deref(), Some(Headless(true)));
    let has_video = video_req.is_some();
    let Some(mesh_assets) = mesh_assets else { return; };
    if !(is_headless && has_video) { return; }

    let x = 0.0;
    let y = 0.25 * WALL_HEIGHT;
    let material = materials.add(ColorMaterial::from(Color::hsl(120.0, 1.0, 0.5)));

    commands.spawn((
        Mesh2d(mesh_assets.ball_circle.clone()),
        MeshMaterial2d(material),
        bevy::render::view::Visibility::Visible,
        bevy::render::view::InheritedVisibility::VISIBLE,
        Transform::from_xyz(x, y, 2.0),
        RenderLayers::layer(1),
    ));
    eprintln!("[diag] spawned headless test ball at ({:.1},{:.1})", x, y);
    *spawned = true;
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
    mut rng_resource: ResMut<RngResource>,
) {
    let rng = &mut rng_resource.rng;
    let mut add_wall = |size: Vec2, position: Vec2| {
        commands.spawn((
            Wall::default(),
            Collider::cuboid(size.x / 2.0, size.y / 2.0),
            Mesh2d(meshes.add(bevy::math::primitives::Rectangle::from_size(size))),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0)))),
            bevy::render::view::Visibility::Visible,
            bevy::render::view::InheritedVisibility::VISIBLE,
            Transform::from_xyz(position.x, position.y, 0.0),
            RenderLayers::layer(1),
        ));
    };
    println!("Setting up whirl");
    const VERTICAL_WALLS_SIZE: Vec2 = Vec2::new(WALL_BOX.max_x - WALL_BOX.min_x, WALL_THICKNESS);
    const HORIZONTAL_WALLS_SIZE: Vec2 = Vec2::new(WALL_THICKNESS, WALL_BOX.max_y - WALL_BOX.min_y);
    add_wall(
        VERTICAL_WALLS_SIZE,
        Vec2 {
            x: 0.0,
            y: WALL_BOX.min_y,
        },
    );
    add_wall(VERTICAL_WALLS_SIZE, Vec2::new(0.0, WALL_BOX.max_y));
    add_wall(HORIZONTAL_WALLS_SIZE, Vec2::new(WALL_BOX.min_x, 0.0));
    add_wall(HORIZONTAL_WALLS_SIZE, Vec2::new(WALL_BOX.max_x, 0.0));

    const HORIZONTAL_SPACING: f32 = 0.5 * PIXELS_PER_METER;
    const VERTICAL_SPACING: f32 = 0.5 * PIXELS_PER_METER;
    const SPACED_WIDTH: i32 = (GROUND_WIDTH / HORIZONTAL_SPACING) as i32;
    const SPACED_HEIGHT: i32 = (WALL_HEIGHT / VERTICAL_SPACING) as i32;

    for i in 0..SPACED_WIDTH {
        let x = (i as f32 * HORIZONTAL_SPACING) - (0.5 * GROUND_WIDTH);
        for j in 1..SPACED_HEIGHT {
            let y = j as f32 * VERTICAL_SPACING + GROUND_POSITION;
            let row_shift: f32 = if j % 2 == 1 {
                0.0
            } else {
                HORIZONTAL_SPACING / 2.0
            };
            if i == 0 && row_shift == 0.0 {
                continue;
            }
            let size = Vec2::new(BALL_RADIUS, BALL_RADIUS);
            let position = Vec2::new(x + row_shift, y);
            commands.spawn((
                Peg::default(),
                Collider::cuboid(size.x / 2.0, size.y / 2.0),
                Mesh2d(meshes.add(bevy::math::primitives::Rectangle::from_size(size))),
                MeshMaterial2d(materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0)))),
                // Raise pegs to z=2 so they are above the walls (z=0) like balls
                Transform::from_xyz(position.x, position.y, 2.0),
            ));
            let is_pocket = (i != SPACED_WIDTH) && rng.gen_range(0.0, 1.0) < 0.025;
            if is_pocket {
                let size = Vec2::new(BALL_RADIUS * 6.0, BALL_RADIUS);
                let position = Vec2::new(
                    x + row_shift + (HORIZONTAL_SPACING / 2.0),
                    y - (VERTICAL_SPACING / 3.0),
                );
                commands.spawn((
                    Peg::default(),
                    Collider::cuboid(size.x / 2.0, size.y / 2.0),
                    Mesh2d(meshes.add(bevy::math::primitives::Rectangle::from_size(size))),
                    MeshMaterial2d(materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0)))),
                    Transform::from_xyz(position.x, position.y, 0.0),
                ));
            }
        }
    }
}

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RngResource {
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
        .add_systems(Startup, setup_meshes)
        .add_systems(Startup, setup_graphics)
        .add_systems(PostUpdate, crate::camera_tuning::set_ortho_scale_after_spawn)
        .add_systems(Startup, setup_whirl)
        .add_systems(Update, spawn_headless_test_ball)
        // TEMP DEBUG: wiggle the offscreen overlay slightly to prove dynamic rendering in offscreen target
        .add_systems(Update, wiggle_offscreen_overlay);
    }
}
