use bevy::{
    prelude::{
        App,
        Assets,
        Camera,
        Camera2d,
        Color,
        Commands,
        Component,
        Handle,
        Image,
        Mesh,
        Plugin,
        Query,
        Res,
        ResMut,
        Resource,
        Startup,
        Transform,
        Vec2,
        With,
    },
    render::{
        mesh::Mesh2d,
        render_resource::{
            Extent3d,
            TextureDimension,
            TextureFormat,
            TextureUsages,
        },
    },
    sprite::{
        ColorMaterial,
        MeshMaterial2d,
    },
};
use bevy_rapier2d::{
    plugin::TimestepMode,
    prelude::{
        Collider,
        NoUserData,
        RapierConfiguration,
        RapierPhysicsPlugin,
    },
};
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::ball::BALL_RADIUS;
use crate::shared_consts::PIXELS_PER_METER;

#[derive(Resource)]
pub struct RngResource {
    pub rng: StdRng,
}

#[derive(Resource, Default)]
pub struct MeshAssets2d {
    pub ball_circle: Handle<Mesh>,
}

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

// Simplified offscreen render target setup based on Bevy's headless example
fn setup_render_target(
    images: &mut ResMut<Assets<Image>>,
    width: u32,
    height: u32,
) -> (bevy::render::camera::RenderTarget, Handle<Image>) {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    // Texture that the camera will render to
    let mut render_target_image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0u8; 4],
        TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::default(),
    );
    render_target_image.texture_descriptor.usage |=
        TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
    let render_target_image_handle = images.add(render_target_image);

    (
        bevy::render::camera::RenderTarget::Image(render_target_image_handle.clone().into()),
        render_target_image_handle,
    )
}

pub fn setup_graphics(
    mut commands: Commands,
    mut rapier_config_q: Query<&mut RapierConfiguration, With<bevy_rapier2d::plugin::context::DefaultRapierContext>>,
    mut timestep_mode: ResMut<TimestepMode>,
    mut images: ResMut<Assets<Image>>,
    video_req: Option<Res<VideoExportRequest>>,
) {
    let has_video = video_req.is_some();
    let export = video_req.as_deref().copied().unwrap_or(VideoExportRequest {
        width: 1080,
        height: 1920,
        fps: 60,
    });

    // Create a simple offscreen render target only when video export is requested
    let mut render_target_opt: Option<bevy::render::camera::RenderTarget> = None;
    if has_video {
        let (rt, img_handle) = setup_render_target(&mut images, export.width, export.height);
        render_target_opt = Some(rt);
        // Provide the render image handle so the capture pipeline can find the GPU image
        commands.insert_resource(crate::capture::RenderImageHandle(img_handle));
        // Mirror VideoExportRequest into CaptureConfig for render app
        commands.insert_resource(crate::capture::CaptureConfig { width: export.width, height: export.height });
    }
    if let Ok(mut rc) = rapier_config_q.single_mut() {
        rc.gravity = Vec2::new(0.0, -9.8 * PIXELS_PER_METER * 0.000_625 * 100.0);
    }
    *timestep_mode = TimestepMode::Fixed {
        dt: 1.0 / 60.0,
        substeps: 1,
    };

    let scale_x = GROUND_WIDTH / export.width as f32;
    let scale_y = WALL_HEIGHT / export.height as f32;
    let fit_scale = scale_x.max(scale_y);
    let center_x = 0.5 * (WALL_BOX.min_x + WALL_BOX.max_x);
    let center_y = 0.5 * (WALL_BOX.min_y + WALL_BOX.max_y);

    // Offscreen camera centered on playfield, with orthographic scale set to fit entire area
    // imports kept near usage for clarity in this function
    use bevy::math::UVec2;
    use bevy::render::camera::{Viewport, ClearColorConfig};
    let mut cam = Camera { hdr: false, order: 1, ..Default::default() };
    if let Some(rt) = render_target_opt {
        cam.target = rt;
    }
    cam.clear_color = ClearColorConfig::Custom(Color::srgba(0.17, 0.18, 0.19, 1.0));
    cam.viewport = Some(Viewport { physical_position: UVec2::new(0, 0), physical_size: UVec2::new(export.width, export.height), depth: 0.0..1.0 });
    let camera_2d = commands.spawn((
        Camera2d,
        cam,
        Transform::from_xyz(center_x, center_y, 1000.0),
    )).id();
    use bevy::render::camera::{Projection, OrthographicProjection};
    let mut ortho = OrthographicProjection::default_2d();
    ortho.scale = fit_scale * 1.0;
    commands.entity(camera_2d).insert(Projection::Orthographic(ortho));
}

pub const WALL_HEIGHT: f32 = 9.0 * PIXELS_PER_METER * 1.620_689_6;
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
            Wall,
            Collider::cuboid(size.x / 2.0, size.y / 2.0),
            Mesh2d(meshes.add(bevy::math::primitives::Rectangle::from_size(size))),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0)))),
            Transform::from_xyz(position.x, position.y, 0.0),
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
                Peg,
                Collider::cuboid(size.x / 2.0, size.y / 2.0),
                Mesh2d(meshes.add(bevy::math::primitives::Rectangle::from_size(size))),
                MeshMaterial2d(materials.add(ColorMaterial::from(Color::hsl(0.0, 0.0, 1.0)))),
                // Raise pegs to z=2 so they are above the walls (z=0) like balls
                Transform::from_xyz(position.x, position.y, 0.0),
            ));
            let is_pocket = (i != SPACED_WIDTH) && rng.gen_range(0.0, 1.0) < 0.025;
            if is_pocket {
                let size = Vec2::new(BALL_RADIUS * 6.0, BALL_RADIUS);
                let position = Vec2::new(
                    x + row_shift + (HORIZONTAL_SPACING / 2.0),
                    y - (VERTICAL_SPACING / 3.0),
                );
                commands.spawn((
                    Peg,
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
        });
        app.add_plugins((
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(PIXELS_PER_METER),
        ));
        app.add_systems(Startup, setup_meshes);
        app.add_systems(Startup, setup_graphics);
        app.add_systems(Startup, setup_whirl);
    }
}
