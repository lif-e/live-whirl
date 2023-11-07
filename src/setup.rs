use bevy::prelude::{
    App,
    // Camera,
    Camera2dBundle,
    Commands,
    // EventReader,
    // OrthographicProjection,
    Plugin,
    // Query,
    ResMut,
    Startup,
    Transform,
    TransformBundle,
    // Update,
    Vec2,
    // With,
};
use bevy_rapier2d::prelude::{
    ActiveEvents,
    Collider,
    // CollisionEvent,
    DebugRenderMode,
    NoUserData,
    RapierConfiguration,
    RapierDebugRenderPlugin,
    RapierPhysicsPlugin,
    // Restitution,
    Sensor,
};

use crate::shared_consts::{PIXELS_PER_METER, GROUND_POSITION};

pub fn setup_graphics(mut commands: Commands, mut rapier_config: ResMut<RapierConfiguration>) {
    rapier_config.gravity = Vec2::new(0.0, -520.0);

    // Add a camera so we can see the debug-render.
    let mut camera_bundle: Camera2dBundle = Camera2dBundle::default();
    camera_bundle.projection.scale = 5.0;
    commands.spawn(camera_bundle);
}

const WALL_THICKNESS: f32 = 0.1 * PIXELS_PER_METER;
const WALLS_HEIGHT: f32 = 5.0 * PIXELS_PER_METER;
const GROUND_WIDTH: f32 = 4.0 * PIXELS_PER_METER;
// const WALL_BOUNCINESS: f32 = 0.90;

pub fn setup_whirl(mut commands: Commands) {
    /* Create the ground. */
    commands
        .spawn(Collider::cuboid(GROUND_WIDTH, WALL_THICKNESS))
        .insert(TransformBundle::from(Transform::from_xyz(0.0, GROUND_POSITION, 0.0)));
    commands
        .spawn(Collider::cuboid(WALL_THICKNESS, WALLS_HEIGHT))
        .insert(TransformBundle::from(Transform::from_xyz(-1.0 * GROUND_WIDTH, WALLS_HEIGHT + GROUND_POSITION, 0.0)));
    commands
        .spawn(Collider::cuboid(WALL_THICKNESS, WALLS_HEIGHT))
        .insert(TransformBundle::from(Transform::from_xyz(GROUND_WIDTH, WALLS_HEIGHT + GROUND_POSITION, 0.0)));
    commands
        .spawn(Collider::cuboid(GROUND_WIDTH, WALL_THICKNESS))
        .insert(Sensor)
        .insert(TransformBundle::from(Transform::from_xyz(0.0, 2.0 * WALLS_HEIGHT + GROUND_POSITION, 0.0)))
        .insert(ActiveEvents::COLLISION_EVENTS);
}

// fn vent_collisions(
//     mut collision_events: EventReader<CollisionEvent>,
// ) {
//     for collision_event in collision_events.iter() {
//         println!("Received collision event: {:?}", collision_event);
//     }
// }

// fn vent_collisions(
//     mut commands: Commands, 
//     query: Query<(Entity, &Transform), With<Sensor>>
// ) {
//     for (entity, position) in query.iter() {
//         if position.translation.y < GROUND_POSITION {
//             println!("Removing");
//             commands.entity(entity).despawn();
//         }
//     }
// }

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins((
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(PIXELS_PER_METER),
            RapierDebugRenderPlugin {
                mode: (
                    DebugRenderMode::COLLIDER_SHAPES
                    // | DebugRenderMode::RIGID_BODY_AXES
                    // | DebugRenderMode::MULTIBODY_JOINTS
                    // | DebugRenderMode::IMPULSE_JOINTS
                    // | DebugRenderMode::JOINTS
                    // | DebugRenderMode::COLLIDER_AABBS
                    // | DebugRenderMode::SOLVER_CONTACTS
                    // | DebugRenderMode::CONTACTS
                ),
                ..RapierDebugRenderPlugin::default()
            },
        ))
        .add_systems(
            Startup,
            (
                setup_graphics,
                setup_whirl,
            ),
        )
        // .add_systems(
        //     Update,
        //     (
        //         vent_collisions,
        //     ),
        // )
        ;
    }
}