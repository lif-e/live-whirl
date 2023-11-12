use rand::{
    Rng,

    thread_rng,
    // rngs::StdRng,
    // SeedableRng,
};

use bevy::{
    prelude::{
        App,
        Assets,
        BuildChildren,
        Children,
        Color,
        Commands,
        Entity,
        EventReader,
        Mesh,
        Plugin,
        Query,
        Res,
        ResMut,
        Resource,
        shape::{
            Circle,
            Box,
        },
        Time,
        Timer,
        TimerMode,
        Transform,
        Update,
        Vec2,
    },
    sprite::{
        ColorMaterial,
        MaterialMesh2dBundle,
    },
};
use bevy_rapier2d::prelude::{
    ActiveEvents,
    Collider,
    ColliderMassProperties,
    ContactForceEvent,
    Friction,
    ImpulseJoint,
    RevoluteJointBuilder,
    RigidBody,
    Velocity,
    RapierContext,
    Restitution,
    RapierImpulseJointHandle,
};

use crate::{
    shared_consts::PIXELS_PER_METER,
    setup::{
        GROUND_WIDTH,
        WALL_THICKNESS,
        WALL_HEIGHT,
    },
};

pub const BALL_RADIUS: f32 = 0.03 * PIXELS_PER_METER;

#[derive(Resource)]
struct NewBallsTimer(pub Timer);

const SPAWN_BOX: Box = Box {
    min_x: (-0.5 * GROUND_WIDTH) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    max_x: ( 0.5 * GROUND_WIDTH) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
    min_y: (-0.5 *  WALL_HEIGHT) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    max_y: ( 0.5 *  WALL_HEIGHT) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
    min_z: 0.0,
    max_z: 0.0,
};

const MIN_LINEAR_VELOCITY: Vec2 = Vec2::new(-1.0, -1.0);
const MAX_LINEAR_VELOCITY: Vec2 = Vec2::new( 1.0,  1.0);

const STICKY_BREAKING_FORCE: f32 = 0.0000025;

fn add_balls(
    time: Res<Time>,
    mut timer: ResMut<NewBallsTimer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) { 
    // let mut rng = StdRng::seed_from_u64(42);
    let mut rng = thread_rng();

    let linearvelocity: Vec2 = Vec2::new(
        rng.gen_range(MIN_LINEAR_VELOCITY.x, MAX_LINEAR_VELOCITY.x),
        rng.gen_range(MIN_LINEAR_VELOCITY.y, MAX_LINEAR_VELOCITY.y),
    );
    let max_linear_magnitude: f32 = MAX_LINEAR_VELOCITY.length().abs().max(MAX_LINEAR_VELOCITY.length().abs());
    let magnitude: f32 = linearvelocity.length().abs();

    // update our timer with the time elapsed since the last update
    // if that caused the timer to finish, we say hello to everyone
    if timer.0.tick(time.delta()).just_finished() {
        commands
            .spawn((
                RigidBody::Dynamic,
                Collider::ball(BALL_RADIUS),
                ColliderMassProperties::Density(0.001),
                Friction::coefficient(0.7),
                Velocity {
                    linvel: linearvelocity * PIXELS_PER_METER,
                    angvel: 0.0,
                },
                ActiveEvents::CONTACT_FORCE_EVENTS,
                // Ccd::enabled(),
                Restitution::new(0.1),
                
                MaterialMesh2dBundle {
                    mesh: meshes.add(Circle::new(BALL_RADIUS).into()).into(),
                    // 4. Put something bright in a dark environment to see the effect
                    material: materials.add(ColorMaterial::from(
                        Color::hsl(
                            rng.gen_range(0.0, 360.0),
                            ((magnitude / max_linear_magnitude) * 10.0) + (0.90 - 0.11),
                            0.5,
                        )
                    )),
                    transform: Transform::from_xyz(
                        rng.gen_range(SPAWN_BOX.min_x, SPAWN_BOX.max_x),
                        rng.gen_range(SPAWN_BOX.min_y, SPAWN_BOX.max_y),
                        0.0,
                    ),
                    ..MaterialMesh2dBundle::default()
                },
            ));
    }
}

const MAX_JOINTS: usize = 10;
const JOINT_DISTANCE: f32 = BALL_RADIUS * 0.05;

fn sticky(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut contact_force_collisions: EventReader<ContactForceEvent>,
    parent_entities: Query<(Entity, &Children)>,
) {
    for ContactForceEvent{collider1, collider2, total_force_magnitude, ..} in contact_force_collisions.iter() {
        if *total_force_magnitude > STICKY_BREAKING_FORCE { continue; }
        if let Some(contact_pair) = rapier_context.contact_pair(*collider1, *collider2) {
            // The contact pair exists meaning that the broad-phase identified a potential contact.
            if contact_pair.has_any_active_contacts() {
                // There's only ever really one contact manifold for pure circles.
                for manifold in contact_pair.manifolds() {
                    // There's only ever really one point pair for pure circles.
                    for contact_point in manifold.points() {
                        // Keep in mind that all the geometric contact data are expressed in the local-space of the colliders.
                        let e1_sticky_point: Vec2 = contact_point.local_p1().normalize_or_zero() * (BALL_RADIUS + JOINT_DISTANCE);
                        let e2_sticky_point: Vec2 = contact_point.local_p2().normalize_or_zero() * (BALL_RADIUS + JOINT_DISTANCE);
                        if let Ok(c1_children) = parent_entities.get_component::<Children>(*collider1) {
                            if c1_children.len() > MAX_JOINTS { continue;}
                        } else if let Ok(c2_children) = parent_entities.get_component::<Children>(*collider2) {
                            if c2_children.len() > MAX_JOINTS { continue; }
                        }

                        // joint.set_contacts_enabled(false);
                        commands
                            .entity(*collider2)
                            .with_children(|children| {
                                children
                                    .spawn(ImpulseJoint::new(
                                        *collider1,
                                        RevoluteJointBuilder::new()
                                            .local_anchor1(e1_sticky_point)
                                            .local_anchor2(e2_sticky_point)
                                            .build(),
                                    ))
                                ;
                            })
                        ;
                    }
                }
            }
    
        }
    }
}

fn unstick(
    mut commands: Commands,
    mut context: ResMut<RapierContext>,
    mut joints: Query<(Entity, &RapierImpulseJointHandle)>,
) {
    for (joint_entity, rapier_joint_handle) in joints.iter_mut() {
        if let Some(rapier_joint) = context.impulse_joints.get_mut(rapier_joint_handle.0) {
            for impulse in rapier_joint.impulses.column_iter() {
                let impulse_magnitude: f32 = Vec2::new(impulse.x, impulse.y).length();
                if impulse_magnitude > STICKY_BREAKING_FORCE {
                    commands.entity(joint_entity).despawn();
                }
            }
        }
    }
}

pub struct BallPlugin;

impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app
        .insert_resource(
            NewBallsTimer(Timer::from_seconds(0.025, TimerMode::Repeating))
        )
        .add_systems(
            Update,
            (
                add_balls,
                sticky,
                unstick,
            ),
        );
    }
}