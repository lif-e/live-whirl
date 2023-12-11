use rand::{
    Rng, rngs::StdRng,
    // rngs::StdRng,
    // SeedableRng,
};
use std::f32::consts::PI;

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
    },
    sprite::{
        ColorMaterial,
        MaterialMesh2dBundle,
    },
    asset::Handle,
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
    QueryFilter,
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

fn has_too_many_adjacent_joints(
    joint_querier: &Query<&BevyImpulseJoint>,
    children: &Children,
) -> bool {
    let mut joint_count: u8 = 0;
    for child in children.iter() {
        if joint_querier.get(*child).is_err() { continue; }
        joint_count += 1;
        if joint_count >= 5 { return true; }
    }
    return false;
}

fn get_next_ball_position(
    rng: &mut StdRng,
    rapier_context: &Res<RapierContext>,
    x: f32,
    y: f32,
    radius: f32,
    new_ball_radius: f32,
) -> Option<(f32, f32, f32, f32)> {
    let starting_angle = rng.gen_range(0.0, 2.0 * PI);
    for test_angle_ndx in 0..5 {
        let angle = starting_angle + (test_angle_ndx as f32 * (PI / 3.0));
        let total_radius = radius + new_ball_radius;
        let joint_x = x + radius * angle.cos();
        let joint_y = y + radius * angle.sin();
        let new_ball_x = x + total_radius * angle.cos();
        let new_ball_y = y + total_radius * angle.sin();

        let circle_shape = Collider::ball(new_ball_radius);

        // Perform the proximity query
        let first_hit = rapier_context.intersection_with_shape(
            Vec2::new(new_ball_x, new_ball_y),
            angle,
            &circle_shape,
            QueryFilter::default(),
        );
        if first_hit.is_some() { continue; }

        return Some((joint_x, joint_y, new_ball_x, new_ball_y));
    }
    return None;

}

fn reproduce_balls(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    time: Res<Time>,
    mut timer: ResMut<ReproduceBallsTimer>,
    mut rng_resource: ResMut<RngResource>,

    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    q_children_and_transform_and_collider_and_color_handles_with_balls: Query<(&Children, &Transform, &Collider, &Handle<ColorMaterial>), With<Ball>>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
) {
    if !timer.0.tick(time.delta()).just_finished() { return; }
    
    let rng = &mut rng_resource.rng;

    for (children, transform, collider, color_handle) in q_children_and_transform_and_collider_and_color_handles_with_balls.iter() {
        if rng.gen_range(0.0, 1.0) > 0.00625 { continue; }
        if has_too_many_adjacent_joints(&q_bevy_impulse_joints, children) { continue; }
        
        let x = transform.translation.x;
        let y = transform.translation.y;
        let radius = collider.as_ball().unwrap().radius();
        let new_ball_radius: f32 = BALL_RADIUS;
        
        let (_joint_x, _joint_y, new_ball_x, new_ball_y) = match get_next_ball_position(
            rng,
            &rapier_context,
            x,
            y,
            radius,
            new_ball_radius,
        ) {
            Some((joint_x, joint_y, new_ball_x, new_ball_y)) => (joint_x, joint_y, new_ball_x, new_ball_y),
            None => continue,
        };
        // println!("{:#?}", world.inspect_entity(entity));

        let linearvelocity: Vec2 = Vec2::new(0.0, 0.0);

        // let max_linear_magnitude: f32 = MAX_LINEAR_VELOCITY.length().abs().max(MAX_LINEAR_VELOCITY.length().abs());
        // let magnitude: f32 = linearvelocity.length().abs();

        let parent_color_material = color_materials.get(color_handle).unwrap();
        let child_color_material = parent_color_material.clone();

        commands.spawn(
            (
                Ball::default(),
                RigidBody::Dynamic,
                Collider::ball(radius),
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
                    mesh: meshes.add(Circle::new(radius).into()).into(),
                    // 4. Put something bright in a dark environment to see the effect
                    material: color_materials.add(child_color_material),
                    transform: Transform::from_xyz(
                        new_ball_x,
                        new_ball_y,
                        0.0,
                    ),
                    ..MaterialMesh2dBundle::default()
                },
            ),
        );
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
    if !timer.0.tick(time.delta()).just_finished() { return; }

    let rng = &mut rng_resource.rng;
    let linearvelocity: Vec2 = Vec2::new(
        rng.gen_range(MIN_LINEAR_VELOCITY.x, MAX_LINEAR_VELOCITY.x),
        rng.gen_range(MIN_LINEAR_VELOCITY.y, MAX_LINEAR_VELOCITY.y),
    );
    let max_linear_magnitude: f32 = MAX_LINEAR_VELOCITY.length().abs().max(MAX_LINEAR_VELOCITY.length().abs());
    let magnitude: f32 = linearvelocity.length().abs();

    // update our timer with the time elapsed since the last update
    // if that caused the timer to finish, we say hello to everyone
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

const MAX_JOINTS: usize = 10;
const PAIRWISE_JOINTS_ALLOWED: u8 = 2;
const JOINT_DISTANCE: f32 = BALL_RADIUS * 0.05;

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

fn contacts(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut contact_force_collisions: EventReader<ContactForceEvent>,
    q_balls: Query<Entity, With<Ball>>,
    q_children_for_balls: Query<&Children, With<Ball>>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
    q_velocities: Query<&Velocity>,
    color_materials: ResMut<Assets<ColorMaterial>>,
    q_color_material_handles: Query<&Handle<ColorMaterial>>,
) {
    for ContactForceEvent{collider1, collider2, total_force_magnitude, ..} in contact_force_collisions.iter() {
        if *total_force_magnitude > STICKY_BREAKING_FORCE {
            if q_balls.get(*collider1).is_err() || q_balls.get(*collider2).is_err() { continue; }
            // let [velocity1, velocity2] = match q_velocities.get_many([*collider1, *collider2]) {
            //     Ok(velocities) => velocities,
            //     Err(_) => continue,
            // };
            // let [color_material_handle1, color_material_handle2] = match q_color_material_handles.get_many([*collider1, *collider2]) {
            //     Ok(color_material_handles) => color_material_handles,
            //     Err(_) => continue,
            // };
            // let color_material1 = match color_materials.get(color_material_handle1) {
            //     Some(color_material) => color_material,
            //     None => continue,
            // };
            // let color_material2 = match color_materials.get(color_material_handle2) {
            //     Some(color_material) => color_material,
            //     None => continue,
            // };
            // if color_material1.color != color_material2.color {
            //     if velocity1.linvel.length().abs() > velocity2.linvel.length().abs() {
            //         commands.entity(*collider2).despawn();
            //     } else {
            //         commands.entity(*collider1).despawn();
            //     }
            // }
            continue;
        }
    
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
            NewBallsTimer(Timer::from_seconds(3.0, TimerMode::Repeating)),
            // NewBallsTimer(Timer::from_seconds(1.5, TimerMode::Repeating)),
        )
        .insert_resource(
            ReproduceBallsTimer(Timer::from_seconds(0.025, TimerMode::Repeating)),
        )
        .add_systems(
            Update,
            (
                add_balls,
                contacts,
                unstick,
                reproduce_balls,
            ),
        );
    }
}