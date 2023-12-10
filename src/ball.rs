use rand::{
    Rng,
    // rngs::StdRng,
    // SeedableRng,
    seq::IteratorRandom,
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
        Component,
        World,
    },
    sprite::{
        ColorMaterial,
        MaterialMesh2dBundle,
    }, hierarchy::Parent,
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
    // FixedJointBuilder,
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
const JOINT_DISTANCE: f32 = BALL_RADIUS * 0.05;

fn print_component_names(world: &World, entity: Entity, indents: Option<usize>) {
    let indent = "\t".repeat(indents.unwrap_or(0));
    println!("{}Entity: {:?}", indent, entity);
    if let Some(entity_ref) = world.get_entity(entity) {
        for type_id in entity_ref.archetype().components() {
            let component_info = world.components().get_info(type_id).unwrap();
            println!("{}\tComponent: {:?}", indent, component_info.name());
        }
    } else {
        println!("{}Entity not found", indent);
    }
}

fn sticky(
    world: &World,
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut contact_force_collisions: EventReader<ContactForceEvent>,
    balls_with_children: Query<(Entity, &Ball, &Children)>,
    joints: Query<(Entity, &Parent, &BevyImpulseJoint)>,
) {
    for ContactForceEvent{collider1, collider2, total_force_magnitude, ..} in contact_force_collisions.iter() {
        if *total_force_magnitude > STICKY_BREAKING_FORCE { continue; }
        // println!("\nA contact was below the breaking force: {:?}", total_force_magnitude);
        if let Some(contact_pair) = rapier_context.contact_pair(*collider1, *collider2) {
            if *collider1 == *collider2 {
                println!("Yo! This is a self contact!? wth?");
            }
            // println!("\tFound a contact pair between {:?} and {:?}", collider1, collider2);
            // The contact pair exists meaning that the broad-phase identified a potential contact.
            if contact_pair.has_any_active_contacts() {
                // println!("\t\tThe contact pair has active contacts");
                // There's only ever really one contact manifold for pure circles.
                if let Some(manifold) = contact_pair.manifolds().into_iter().next() {
                    // println!("\t\tThe contact pair has at least one contact manifold");
                    // There's only ever really one point pair for pure circles.
                    if let Some(contact_point) = manifold.points().into_iter().next() {
                        // println!("\t\tThe contact pair has at least one contact point");
                        if let Ok(c1_children) = balls_with_children.get_component::<Children>(*collider1) {
                            if c1_children.len() > MAX_JOINTS { continue; }
                            for child in c1_children.iter() {
                                if let Ok(bevy_impulse_joint_parent) = joints.get_component::<Parent>(*child) {
                                    println!("checking {:?} == {:?}", bevy_impulse_joint_parent.get(), *collider2);
                                    if bevy_impulse_joint_parent.get() == *collider2 {
                                        continue;
                                    }
                                }
                            }
                            print_component_names(world, *collider1, Some(0));
                            for child in c1_children.iter() {
                                print_component_names(world, *child, Some(3));
                                if let Ok(bevy_impulse_joint_parent) = joints.get_component::<Parent>(*child) {
                                    println!("\t\t\t\t{:?}", bevy_impulse_joint_parent.get());
                                }
                            }
                        } else if let Ok(c2_children) = balls_with_children.get_component::<Children>(*collider2) {
                            if c2_children.len() > MAX_JOINTS { continue; }
                            for child in c2_children.iter() {
                                if let Ok(bevy_impulse_joint_parent) = joints.get_component::<Parent>(*child) {
                                    println!("checking {:?} == {:?}", bevy_impulse_joint_parent.get(), *collider1);
                                    if bevy_impulse_joint_parent.get() == *collider1 {
                                        continue;
                                    }
                                }
                            }
                            print_component_names(world, *collider1, Some(0));
                            for child in c2_children.iter() {
                                print_component_names(world, *child, Some(3));
                                if let Ok(bevy_impulse_joint_parent) = joints.get_component::<Parent>(*child) {
                                    println!("\t\t\t\t{:?}", bevy_impulse_joint_parent.get());
                                }
                            }
                        }
                        
                        // println!("Creating a new joint between collider {:?} and collider {:?}", collider1, collider2);
                        print!("+");
                        println!("Creating a new joint between collider {:?} and collider {:?}", collider1, collider2);
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
                                    // Explodes Too Often for Some Reason
                                    // .spawn(BevyImpulseJoint::new(
                                    //     *collider1,
                                    //     // NOTE: setting the local anchors sets the translation part of the local frames.
                                    //     FixedJointBuilder::new()
                                    //         .local_anchor1(e1_sticky_point)
                                    //         .local_anchor2(e2_sticky_point)
                                    //         .build(),
                                    // ))
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
    // world: &World,
    mut commands: Commands,
    context: ResMut<RapierContext>,
    balls_with_children: Query<(Entity, &Ball, &Children)>,
    joints: Query<(Entity, &mut BevyImpulseJoint, &RapierImpulseJointHandle)>,
) {
    // for (joint_entity, bevy_impulse_joint, rapier_joint_handle) in joints.iter_mut() {
    //     if let Some(rapier_joint) = context.impulse_joints.get_mut(rapier_joint_handle.0) {
    //         for impulse in rapier_joint.impulses.column_iter() {
    //             let impulse_magnitude: f32 = Vec2::new(impulse.x, impulse.y).length();
    //             if impulse_magnitude > STICKY_BREAKING_FORCE {
    //                 if let Ok(children) = balls_with_children.get_component::<Children>(joint_entity) {
    //                     for child_entity in children.iter() {
    //                         println!("\t{:?} -> {:?}", joint_entity, child_entity);
    //                         print_component_names(world, *child_entity);
    //                     }
    //                 }
    //                 commands.entity(joint_entity).despawn();
    //             }
    //         }
    //     }
    // }
    for (_, _, children) in balls_with_children.iter() {
        for child_entity in children.iter() {
            if let Ok(_bevy_joint) = joints.get_component::<BevyImpulseJoint>(*child_entity) {
                let bevy_joint_entity = child_entity;
                if let Ok(rapier_handle) = joints.get_component::<RapierImpulseJointHandle>(*child_entity) {
                    if let Some(rapier_joint) = context.impulse_joints.get(rapier_handle.0) {
                        for impulse in rapier_joint.impulses.column_iter() {
                            let impulse_magnitude: f32 = Vec2::new(impulse.x, impulse.y).length();
                            if impulse_magnitude > STICKY_BREAKING_FORCE {
                                print!("-");
                                commands.entity(*bevy_joint_entity).despawn();
                            }
                        }
                    
                    }
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
            // NewBallsTimer(Timer::from_seconds(0.025, TimerMode::Repeating)),
            NewBallsTimer(Timer::from_seconds(1.5, TimerMode::Repeating)),
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