use rand::{
    Rng,
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
        Component,
        DespawnRecursiveExt,
        Entity,
        EventReader,
        Mesh,
        Plugin,
        Query,
        Res,
        ResMut,
        Resource,
        shape::{
            Box,
            Circle,
        },
        Time,
        Timer,
        TimerMode,
        Transform,
        Update,
        Vec2,
        With,
        World,
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
    ImpulseJoint as BevyImpulseJoint,
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
        RngResource,
    },
};

pub const BALL_RADIUS: f32 = 0.05 * PIXELS_PER_METER;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct Ball;

impl Default for Ball {
    fn default() -> Self {
        Self
    }
}

#[derive(Resource)]
struct ReproduceBallsTimer(pub Timer);

fn reproduce_balls(
    time: Res<Time>,
    mut timer: ResMut<ReproduceBallsTimer>,
    mut rng_resource: ResMut<RngResource>,
    mut balls: Query<(Entity, &Ball, &Children)>,
    joint_querier: Query<&BevyImpulseJoint>,
    ball_querier: Query<&Ball>,
) {
    let mut rng = &mut rng_resource.rng;
    if timer.0.tick(time.delta()).just_finished() {
        
    }
}

#[derive(Resource)]
struct NewBallsTimer(pub Timer);

const SPAWN_BOX: Box = Box {
    min_x: (-0.5 *  GROUND_WIDTH) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    max_x: ( 0.5 *  GROUND_WIDTH) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
    min_y: (-0.5 *   WALL_HEIGHT) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    max_y: ( 0.5 *   WALL_HEIGHT) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
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
    mut rng_resource: ResMut<RngResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let rng = &mut rng_resource.rng;
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
                Ball::default(),
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
const PAIRWISE_JOINTS_ALLOWED: u8 = 2;
const JOINT_DISTANCE: f32 = BALL_RADIUS * 0.05;

fn print_entity_info(
    world: &World,
    q_children: &Query<&Children>,
    q_bevy_impulse_joint: &Query<&BevyImpulseJoint>,
    entity: Entity,
    label: Option<&str>,
    indents: Option<usize>,
) {
    let defaulted_indents = indents.unwrap_or(0);
    let defaulted_label = label.unwrap_or("Entity");
    println!("{}{}: {:?}", "\t".repeat(defaulted_indents), defaulted_label, entity);
    if let Some(entity_ref) = world.get_entity(entity) {
        for type_id in entity_ref.archetype().components() {
            let component_info = world.components().get_info(type_id).unwrap();
            println!("{}\tComponent: {:?}", "\t".repeat(defaulted_indents + 1), component_info.name());
        }
        if let Ok(children) = q_children.get(entity) {
            for child in children.iter() {
                print_entity_info(world, q_children, q_bevy_impulse_joint, *child, Some("Child"), Some(defaulted_indents + 2));
            }
        }
        if let Ok(bevy_impulse_joint) = q_bevy_impulse_joint.get(entity) {
            print_entity_info(world, q_children, q_bevy_impulse_joint, bevy_impulse_joint.parent, Some("Joint Parent"), Some(defaulted_indents + 2));
        }
    } else {
        println!("{}Entity not found", "\t".repeat(defaulted_indents + 1));
    }
}

fn has_more_than_max_joints(
    q_children_for_balls: &Query<&Children, With<Ball>>,
    collider: &Entity,
) -> bool {
    let children = match q_children_for_balls.get(*collider) {
        Ok(children) => children,
        Err(_) => return false,
    };
    return children.len() > MAX_JOINTS;
}

fn already_has_max_pairwise_joints(
    q_children_for_balls: &Query<&Children, With<Ball>>,
    q_bevy_impulse_joints: &Query<&BevyImpulseJoint>,
    collider1: &Entity,
    collider2: &Entity,
) -> bool {
    let c1_children = match q_children_for_balls.get(*collider1) {
        Ok(c1_children) => c1_children,
        Err(_) => return false,
    };
    let mut count: u8 = 0;
    for child in c1_children.iter() {
        let bevy_impulse_joint = match q_bevy_impulse_joints.get(*child) {
            Ok(bevy_impulse_joint) => bevy_impulse_joint,
            Err(_) => continue,
        };
        if bevy_impulse_joint.parent == *collider2 {
            count += 1;
            if count >= PAIRWISE_JOINTS_ALLOWED { return true; }
        }
    }
    return false;
}

fn sticky(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut contact_force_collisions: EventReader<ContactForceEvent>,
    q_balls: Query<Entity, With<Ball>>,
    q_children_for_balls: Query<&Children, With<Ball>>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
) {
    for ContactForceEvent{collider1, collider2, total_force_magnitude, ..} in contact_force_collisions.iter() {
        if *total_force_magnitude > STICKY_BREAKING_FORCE { continue; }
        if q_balls.get(*collider1).is_err() || q_balls.get(*collider2).is_err() { continue; }
    
        // If the contact pair exists, that means that the broad-phase identified a potential contact.
        let contact_pair = match rapier_context.contact_pair(*collider1, *collider2) {
            Some(contact_pair) => contact_pair,
            None => continue,
        };
        
        if !contact_pair.has_any_active_contacts() { continue; }
    
        // There's only ever really one contact manifold for pure circles, just get the first.
        let manifold = match contact_pair.manifolds().into_iter().next() {
            Some(manifold) => manifold,
            None => continue,
        };
        // There's only ever really one point pair for pure circles, just get the first.
        let contact_point = match manifold.points().into_iter().next() {
            Some(contact_point) => contact_point,
            None => continue,
        };

        if
            has_more_than_max_joints(&q_children_for_balls, collider1) ||
            has_more_than_max_joints(&q_children_for_balls, collider2) ||
            already_has_max_pairwise_joints(&q_children_for_balls, &q_bevy_impulse_joints, collider1, collider2)
        {
            continue;
        }
        // println!("Creating a new joint between collider {:?} and collider {:?}", collider1, collider2);
        // joint.set_contacts_enabled(false);
        commands
            .entity(*collider2)
            .with_children(|parent| {
                    // Keep in mind that all the geometric contact data are expressed in the local-space of the colliders.
                    let e1_sticky_point: Vec2 = contact_point.local_p1().normalize_or_zero() * (BALL_RADIUS + JOINT_DISTANCE);
                    let e2_sticky_point: Vec2 = contact_point.local_p2().normalize_or_zero() * (BALL_RADIUS + JOINT_DISTANCE);
                    parent
                        .spawn(BevyImpulseJoint::new(
                            *collider1,
                            RevoluteJointBuilder::new()
                                .local_anchor1(e1_sticky_point)
                                .local_anchor2(e2_sticky_point)
                                .build(),
                        ))
                    ;
                });
    }
}

fn unstick(
    mut commands: Commands,
    context: ResMut<RapierContext>,
    q_children_for_balls_with_children: Query<&Children, With<Ball>>,
    q_rapier_handles_with_bevy_impulse_joints: Query<&RapierImpulseJointHandle, With<BevyImpulseJoint>>,
) {
    for ball_children in q_children_for_balls_with_children.iter() {
        for child_entity in ball_children.iter() {
            let rapier_handle = match q_rapier_handles_with_bevy_impulse_joints.get(*child_entity) {
                Ok(rapier_handle) => rapier_handle,
                Err(_) => continue,
            };
            let bevy_impulse_joint_entity = child_entity;
            let rapier_joint = match context.impulse_joints.get(rapier_handle.0) {
                Some(rapier_joint) => rapier_joint,
                None => continue,
            };
            for impulse in rapier_joint.impulses.column_iter() {
                let impulse_magnitude: f32 = Vec2::new(impulse.x, impulse.y).length();
                if impulse_magnitude > STICKY_BREAKING_FORCE {
                    commands.entity(*bevy_impulse_joint_entity).despawn_recursive();
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
            NewBallsTimer(Timer::from_seconds(0.025, TimerMode::Repeating)),
            // NewBallsTimer(Timer::from_seconds(1.5, TimerMode::Repeating)),
        )
        .insert_resource(
            ReproduceBallsTimer(Timer::from_seconds(3.0, TimerMode::Repeating)),
        )
        .add_systems(
            Update,
            (
                add_balls,
                sticky,
                unstick,
                // reproduce_balls,
            ),
        );
    }
}