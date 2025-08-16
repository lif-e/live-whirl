use rand::{
    rngs::StdRng,
    // rngs::StdRng,
    // SeedableRng,
    Rng, SeedableRng,
};
use std::f32::consts::PI;

use bevy::{
    asset::Handle,
    color::Hsla,
    hierarchy::Parent,
    math::primitives::Circle,
    prelude::{
        App, Assets, BuildChildren, Children, Color, Commands, Component, DespawnRecursiveExt,
        Entity, EventReader, Mesh, Plugin, Query, Res, ResMut, Resource, Time, Timer, TimerMode,
        Transform, Update, Vec2, With,
    },
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, ColliderMassProperties, ContactForceEvent, Friction,
    ImpulseJoint as BevyImpulseJoint, QueryFilter, RapierContext, RapierImpulseJointHandle,
    Restitution, RevoluteJointBuilder, RigidBody, Velocity,
};

use crate::{
    setup::{RngResource, GROUND_WIDTH, WALL_HEIGHT, WALL_THICKNESS},
    shared_consts::PIXELS_PER_METER,
};

pub const BALL_RADIUS: f32 = 0.05 * PIXELS_PER_METER;
const MAX_LIFE_POINTS: u32 = u32::MAX / (2 as u32).pow(32 - 10);
const COLOR_SATURATION_SCALE_FACTOR: f32 = 10.0;
const COLOR_SATURATION_MINIMUM: f32 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct Ball {
    pub age: u32,
    pub life_points: u32,
    pub genome_max_age: u32,
    pub genome_relative_reproduction_rate: f32,
    pub genome_bite_size: u32,
    pub genome_life_points_safe_to_reproduce: u32,
    pub genome_energy_share_with_children: f32,
    pub genome_friendly_scent: Vec2,
    pub genome_friendly_distance: f32,
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            age: 0,
            life_points: MAX_LIFE_POINTS,
            genome_max_age: 90,
            genome_relative_reproduction_rate: 0.00625 * 2.0,
            genome_bite_size: 100,
            genome_life_points_safe_to_reproduce: 20,
            genome_energy_share_with_children: 0.5,
            genome_friendly_scent: Vec2::new(0.0, 0.0),
            genome_friendly_distance: 0.1,
        }
    }
}

impl Ball {
    fn get_hue(&self) -> f32 {
        let v = self.genome_friendly_scent;
        // Map x from -1..1 to 0..120 (Red to Green)
        let hue_x = ((v.x + 1.0) / 2.0) * 120.0;

        // Map y from -1..1 to 240..60 (Blue to Yellow)
        // This is a bit more complex as it's not a direct opposite on the wheel
        let hue_y = 240.0 - ((v.y + 1.0) / 2.0) * 180.0;

        // Average the two hues for a simple blend
        let hue = (hue_x + hue_y) / 2.0;

        return hue;
    }
    fn get_saturation(&self) -> f32 {
        return ((self.life_points as f32 / MAX_LIFE_POINTS as f32)
            * COLOR_SATURATION_SCALE_FACTOR)
            + COLOR_SATURATION_MINIMUM;
    }
    pub fn transform_color(&self, color: Color) -> Color {
        // Preserve the original color's lightness while applying this Ball's hue/saturation.
        // In Bevy 0.14, use color space structs instead of direct h/s/l accessors.
        // Convert to HSLA, swap h and s, keep l and a as-is.
        let hsla: Hsla = color.into();
        let new_hsla = Hsla {
            hue: self.get_hue().into(),
            saturation: self.get_saturation(),
            lightness: hsla.lightness,
            alpha: hsla.alpha,
        };
        Color::from(new_hsla)
    }
    pub fn get_color(&self) -> Color {
        return Color::hsl(self.get_hue(), self.get_saturation(), 0.5);
    }
    fn is_friendly_with(&self, other: Ball) -> bool {
        let scent_1 = self.genome_friendly_scent;
        let scent_2 = other.genome_friendly_scent;
        let scent_distance = (scent_1 - scent_2).length();
        return scent_distance < self.genome_friendly_distance;
    }
}

fn share_total_roughly(preferred_number: u32, other_number: u32, sharing_rate: f32) -> (u32, u32) {
    let total_life_points: u64 = preferred_number as u64 + other_number as u64;
    let lower_part: u32 = (total_life_points as f32 / (1.0 / sharing_rate)).floor() as u32;
    let higher_part = (total_life_points - lower_part as u64) as u32;
    return (higher_part, lower_part);
}

#[derive(Resource)]
struct ReproduceBallsTimer(pub Timer);

#[derive(Resource)]
struct BallAndJointLoopTimer(pub Timer);

const SURVIVAL_COST: u32 = 1;

fn update_life_points(
    mut commands: Commands,
    mut timer: ResMut<BallAndJointLoopTimer>,
    time: Res<Time>,
    mut q_balls_and_colors: Query<(Entity, &mut Ball, &Handle<ColorMaterial>)>,
    q_impulse_joints: Query<(&BevyImpulseJoint, &Parent)>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut rng_resource: ResMut<RngResource>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for (entity, mut ball, color_handle) in q_balls_and_colors.iter_mut() {
        // let before_life_points = ball.life_points;
        ball.age = if ball.age != u32::MAX {
            ball.age + 1
        } else {
            u32::MAX
        };
        if ball.age > ball.genome_max_age {
            ball.life_points = ball.life_points.checked_sub(SURVIVAL_COST).unwrap_or(0);
        }
        // let change = ball.life_points as i32 - before_life_points as i32;
        // if change != 0 || ball.life_points == 0 {
        //     print!(
        //         "\nLife {: >10}->{: >10} ({: >11}) with age {: >10}/{: >10} ({: >10})",
        //         before_life_points,
        //         ball.life_points,
        //         change,
        //         ball.age,
        //         ball.genome_max_age,
        //         ball.genome_max_age as i32 - ball.age as i32,
        //     );
        // }

        if ball.life_points <= 9 {
            // print!("o");
            commands.entity(entity).despawn_recursive();
        }
        let color_material = match color_materials.get_mut(color_handle) {
            Some(color_material) => color_material,
            None => continue,
        };
        color_material.color = ball.transform_color(color_material.color);
    }

    let rng = &mut rng_resource.rng;

    for (joint, parent) in q_impulse_joints.iter() {
        let [(mut parent_ball, parent_color_handle), (mut child_ball, child_color_handle)] =
            match q_balls_and_colors.get_many_mut([parent.get(), joint.parent]) {
                Ok(
                    [(_, parent_ball, parent_color_handle), (_, child_ball, child_color_handle)],
                ) => [
                    (*parent_ball, parent_color_handle),
                    (*child_ball, child_color_handle),
                ],
                Err(_) => continue,
            };
        // let parent_color_material = match color_materials.get(parent_color_handle) {
        //     Some(color_material) => color_material,
        //     None => continue,
        // };
        // let child_color_material = match color_materials.get(child_color_handle) {
        //     Some(color_material) => color_material,
        //     None => continue,
        // };
        let parent_points: i32 = parent_ball.life_points as i32;
        let life_points_diff_abs = match parent_points.checked_sub(child_ball.life_points as i32) {
            Some(life_points_diff_abs) => life_points_diff_abs.abs(),
            None => i32::MAX,
        };
        let parent_is_friendly = parent_ball.is_friendly_with(child_ball);
        let child_is_friendly = child_ball.is_friendly_with(parent_ball);
        if parent_is_friendly && child_is_friendly && (life_points_diff_abs <= 19) {
            continue;
        }

        let sharing_rate: f32 = if parent_is_friendly && child_is_friendly {
            0.5
        } else if !parent_is_friendly && !child_is_friendly {
            if parent_ball.life_points > child_ball.life_points && life_points_diff_abs > 100 {
                rng.gen_range(0.5, 0.9)
            } else if parent_ball.life_points < child_ball.life_points && life_points_diff_abs > 100 {
                rng.gen_range(0.1, 0.5)
            } else {
                0.5
            }
        } else if !parent_is_friendly && child_is_friendly {
            0.75
        } else if parent_is_friendly && !child_is_friendly {
            0.25
        } else {
            0.5
        };
        (parent_ball.life_points, child_ball.life_points) = share_total_roughly(
            parent_ball.life_points,
            child_ball.life_points,
            sharing_rate,
        );

        let parent_color_material = match color_materials.get_mut(parent_color_handle) {
            Some(color_material) => color_material,
            None => continue,
        };
        parent_color_material.color = parent_ball.transform_color(parent_color_material.color);
        let child_color_material = match color_materials.get_mut(child_color_handle) {
            Some(color_material) => color_material,
            None => continue,
        };
        child_color_material.color = child_ball.transform_color(child_color_material.color);
    }
}

fn has_too_many_adjacent_joints(
    joint_querier: &Query<&BevyImpulseJoint>,
    children: &Children,
) -> bool {
    let mut joint_count: u8 = 0;
    for child in children.iter() {
        if joint_querier.get(*child).is_err() {
            continue;
        }
        joint_count += 1;
        if joint_count >= 5 {
            return true;
        }
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
            angle, // This could just be 0
            &circle_shape,
            QueryFilter::default(),
        );
        if first_hit.is_some() {
            continue;
        }

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
    q_children_and_transform_and_collider_and_color_handles_with_balls: Query<(
        &Children,
        &Transform,
        &Collider,
        &Handle<ColorMaterial>,
        &mut Ball,
        &Velocity,
    )>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let rng = &mut rng_resource.rng;

    for (children, transform, collider, color_handle, parent_ball, parent_ball_velocity) in
        q_children_and_transform_and_collider_and_color_handles_with_balls.iter()
    {
        if rng.gen_range(0.0, 1.0) > parent_ball.genome_relative_reproduction_rate {
            continue;
        }
        if parent_ball.life_points < parent_ball.genome_life_points_safe_to_reproduce {
            continue;
        }
        if has_too_many_adjacent_joints(&q_bevy_impulse_joints, children) {
            continue;
        }

        let x = transform.translation.x;
        let y = transform.translation.y;
        let radius = collider.as_ball().unwrap().radius();
        let new_ball_radius: f32 = BALL_RADIUS;

        let (_joint_x, _joint_y, new_ball_x, new_ball_y) =
            match get_next_ball_position(rng, &rapier_context, x, y, radius, new_ball_radius) {
                Some((joint_x, joint_y, new_ball_x, new_ball_y)) => {
                    (joint_x, joint_y, new_ball_x, new_ball_y)
                }
                None => {
                    if rng.gen_range(0.0, 1.0) < 0.005 {
                        (0.0, 0.0, x, y)
                    } else {
                        continue;
                    }
                }
            };
        // println!("{:#?}", world.inspect_entity(entity));

        let linearvelocity: Vec2 =
            Vec2::new(parent_ball_velocity.linvel.x, parent_ball_velocity.linvel.y);

        let mut parent_ball = *parent_ball;
        let child_life_points;
        (parent_ball.life_points, child_life_points) = share_total_roughly(
            parent_ball.life_points,
            0,
            parent_ball.genome_energy_share_with_children,
        );
        let child_ball = Ball {
            age: 0,
            life_points: child_life_points,
            genome_max_age: ((parent_ball.genome_max_age as f32 + rng.gen_range(-3.0, 3.0))
                .clamp(0.0, u32::MAX as f32) as u32),
            genome_relative_reproduction_rate: parent_ball.genome_relative_reproduction_rate
                + rng.gen_range(-0.01, 0.01),
            genome_bite_size: ((parent_ball.genome_bite_size as f32 + rng.gen_range(-10.0, 10.0))
                .max(0.0)
                .min(u32::MAX as f32) as u32),
            genome_life_points_safe_to_reproduce: ((parent_ball.genome_life_points_safe_to_reproduce
                as f32
                + rng.gen_range(-1000.0, 1000.0))
            .max(0.0)
            .min(u32::MAX as f32) as u32),
            genome_energy_share_with_children: (parent_ball.genome_energy_share_with_children
                + rng.gen_range(-0.1, 0.1))
            .clamp(0.0, 1.0),
            genome_friendly_scent: Vec2::new(
                parent_ball.genome_friendly_scent.x + rng.gen_range(-0.1, 0.1),
                parent_ball.genome_friendly_scent.y + rng.gen_range(-0.1, 0.1),
            ),
            genome_friendly_distance: (parent_ball.genome_friendly_distance
                + rng.gen_range(-0.1, 0.1)),
        };

        let parent_color_material = color_materials.get_mut(color_handle).unwrap();
        parent_color_material.color = parent_ball.transform_color(parent_color_material.color);
        let child_color_material = ColorMaterial::from(child_ball.get_color());

        // print!(
        //     "\nBaby: Life {: >10}, Max Age {: >10}, Reproduction Rate {: >.4}, Bite Size {: >10}, Safe Reproduction Life {: >10}",
        //     child_ball.life_points,
        //     child_ball.genome_max_age,
        //     child_ball.genome_relative_reproduction_rate,
        //     child_ball.genome_bite_size,
        //     child_ball.genome_life_points_safe_to_reproduce,
        // );

        commands.spawn((
            child_ball,
            RigidBody::Dynamic,
            Collider::ball(radius),
            ColliderMassProperties::Density(0.001),
            Friction::coefficient(0.7),
            Velocity {
                linvel: linearvelocity,
                angvel: 0.0,
            },
            ActiveEvents::CONTACT_FORCE_EVENTS,
            // Ccd::enabled(),
            Restitution::new(0.1),
            MaterialMesh2dBundle {
                mesh: meshes.add(Circle::new(radius)).into(),
                // 4. Put something bright in a dark environment to see the effect
                material: color_materials.add(child_color_material),
                transform: Transform::from_xyz(new_ball_x, new_ball_y, 0.0),
                ..MaterialMesh2dBundle::default()
            },
        ));
    }
}

#[derive(Resource)]
struct NewBallsTimer(pub Timer);

struct Box2D { min_x: f32, max_x: f32, min_y: f32, max_y: f32, min_z: f32, max_z: f32 }
const SPAWN_BOX: Box2D = Box2D {
    min_x: (-0.5 * GROUND_WIDTH) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    max_x: (0.5 * GROUND_WIDTH) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
    // min_y: (-0.5 *   WALL_HEIGHT) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    min_y: (0.5 * WALL_HEIGHT) - (BALL_RADIUS * 4.0) - (0.5 * WALL_THICKNESS),
    max_y: (0.5 * WALL_HEIGHT) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
    min_z: 0.0,
    max_z: 0.0,
};

const MIN_LINEAR_VELOCITY: Vec2 = Vec2::new(-1.0, -1.0);
const MAX_LINEAR_VELOCITY: Vec2 = Vec2::new(1.0, 1.0);

// const STICKY_BREAKING_FORCE: f32 = 0.0000025;
// const STICKY_BREAKING_FORCE: f32 = 0.000005;
const STICKY_BREAKING_FORCE: f32 = 0.00001;

fn add_balls(
    time: Res<Time>,
    mut timer: ResMut<NewBallsTimer>,
    mut commands: Commands,
    mut rng_resource: ResMut<RngResource>,
    rapier_context: Res<RapierContext>,
    mesh_assets: Res<crate::setup::MeshAssets2d>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    q_balls: Query<Entity, With<Ball>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let rng = &mut rng_resource.rng;
    let linearvelocity: Vec2 = Vec2::new(
        rng.gen_range(
            MIN_LINEAR_VELOCITY.x, 
            MAX_LINEAR_VELOCITY.x,
        ),
        rng.gen_range(
            MIN_LINEAR_VELOCITY.y, 
            MAX_LINEAR_VELOCITY.y,
        ),
    );

    let ball = Ball {
        age: 0,
        life_points: MAX_LIFE_POINTS,
        genome_max_age: rng.gen_range(90, 120),
        genome_relative_reproduction_rate: rng.gen_range(
            0.00625 * 1.9, 
            0.00625 * 2.0,
        ),
        genome_bite_size: rng.gen_range(0, 400),
        genome_life_points_safe_to_reproduce: rng.gen_range(
            0, 
            1000,
        ),
        genome_energy_share_with_children: rng.gen_range(
            0.25, 
            0.75,
        ),
        genome_friendly_scent: Vec2::new(
            rng.gen_range(-1.0, 1.0), 
            rng.gen_range(-1.0, 1.0),
        ),
        genome_friendly_distance: rng.gen_range(
            0.15, 
            1.0,
        ),
    };
    let color_material: ColorMaterial = ColorMaterial::from(
        ball.get_color(),
    );

    let x = rng.gen_range(
        SPAWN_BOX.min_x, 
        SPAWN_BOX.max_x,
    );
    let y = rng.gen_range(
        SPAWN_BOX.min_y, 
        SPAWN_BOX.max_y,
    );

    let circle_shape = Collider::ball(BALL_RADIUS);

    // Perform the proximity query
    let first_hit = 
        rapier_context.intersection_with_shape(
            Vec2::new(x, y),
            0.0,
            &circle_shape,
            QueryFilter::default(),
        );
    if first_hit.is_some() {
        return;
    }

    // update our timer with the time elapsed since the last update
    // if that caused the timer to finish, we say hello to everyone
    commands.spawn((
        ball,
        RigidBody::Dynamic,
        circle_shape,
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
            mesh: mesh_assets.ball_circle.clone().into(),
            // 4. Put something bright in a dark environment to see the effect
            material: materials.add(color_material),
            transform: Transform::from_xyz(x, y, 0.0),
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
            if count >= PAIRWISE_JOINTS_ALLOWED {
                return true;
            }
        }
    }
    return false;
}

fn contacts(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut contact_force_collisions: EventReader<ContactForceEvent>,
    q_balls: Query<&mut Ball>,
    q_children_for_balls: Query<&Children, With<Ball>>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
    q_velocities: Query<&Velocity>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    q_color_material_handles: Query<&Handle<ColorMaterial>>,
) {
    for ContactForceEvent {
        collider1,
        collider2,
        total_force_magnitude,
        ..
    } in contact_force_collisions.read()
    {
        let collider1 = *collider1;
        let collider2 = *collider2;
        if *total_force_magnitude > STICKY_BREAKING_FORCE {
            let ball1 = q_balls.get(collider1);
            let ball2 = q_balls.get(collider2);
            if ball1.is_err() || ball2.is_err() {
                continue;
            }
            let [velocity1, velocity2] = match q_velocities.get_many([collider1, collider2]) {
                Ok(velocities) => velocities,
                Err(_) => continue,
            };
            let [color_material_handle1, color_material_handle2] =
                match q_color_material_handles.get_many([collider1, collider2]) {
                    Ok(color_material_handles) => color_material_handles,
                    Err(_) => continue,
                };

            let mut ball2 = *ball2.unwrap();
            let mut ball1 = *ball1.unwrap();
            let scent_1 = ball1.genome_friendly_scent;
            let scent_2 = ball2.genome_friendly_scent;
            let scent_distance = (scent_1 - scent_2).length();
            let one_is_friendly = scent_distance < ball1.genome_friendly_distance;
            let two_is_friendly = scent_distance < ball2.genome_friendly_distance;

            if one_is_friendly && two_is_friendly {
                continue;
            }

            if !one_is_friendly
                && (velocity1.linvel.length().abs() > velocity2.linvel.length().abs())
            {
                let bite_size = ball1.genome_bite_size;
                ball2.life_points = if ball2.life_points <= bite_size {
                    0
                } else {
                    ball2.life_points - bite_size
                };
                ball1.life_points = if ball1.life_points >= (u32::MAX - bite_size) {
                    u32::MAX
                } else {
                    ball1.life_points + bite_size
                };
                // print!(
                //     "\nBite {: >10}->{: >10} ({: >11})",
                //     ball2.life_points,
                //     ball1.life_points,
                //     ball1.life_points as i32 - ball2.life_points as i32,
                // );
            } else if !two_is_friendly
                && (velocity2.linvel.length().abs() > velocity1.linvel.length().abs())
            {
                let bite_size = ball2.genome_bite_size;
                ball1.life_points = if ball1.life_points <= bite_size {
                    0
                } else {
                    ball1.life_points - bite_size
                };
                ball2.life_points = if ball2.life_points >= (u32::MAX - bite_size) {
                    u32::MAX
                } else {
                    ball2.life_points + bite_size
                };
                // print!(
                //     "\nBite {: >10}->{: >10} ({: >11})",
                //     ball1.life_points,
                //     ball2.life_points,
                //     ball2.life_points as i32 - ball1.life_points as i32,
                // );
            }
            let color_material1 = color_materials.get_mut(color_material_handle1).unwrap();
            color_material1.color = ball1.transform_color(color_material1.color);
            let color_material2 = color_materials.get_mut(color_material_handle2).unwrap();
            color_material2.color = ball2.transform_color(color_material2.color);
            continue;
        }

        // If the contact pair exists, that means that the broad-phase identified a potential contact.
        let contact_pair = match rapier_context.contact_pair(collider1, collider2) {
            Some(contact_pair) => contact_pair,
            None => continue,
        };

        if !contact_pair.has_any_active_contact() {
            continue;
        }

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

        if has_more_than_max_joints(&q_children_for_balls, &collider1)
            || has_more_than_max_joints(&q_children_for_balls, &collider2)
            || already_has_max_pairwise_joints(
                &q_children_for_balls,
                &q_bevy_impulse_joints,
                &collider1,
                &collider2,
            )
        {
            continue;
        }
        // println!("Creating a new joint between collider {:?} and collider {:?}", collider1, collider2);
        // joint.set_contacts_enabled(false);
        // print!("+");

        let e1_sticky_point: Vec2 =
            contact_point.local_p1().normalize_or_zero() * (BALL_RADIUS + JOINT_DISTANCE);
        let e2_sticky_point: Vec2 =
            contact_point.local_p2().normalize_or_zero() * (BALL_RADIUS + JOINT_DISTANCE);
        let child = commands
            .spawn(BevyImpulseJoint::new(
                collider1,
                RevoluteJointBuilder::new()
                    .local_anchor1(e1_sticky_point)
                    .local_anchor2(e2_sticky_point)
                    .build(),
            ))
            .id();

        commands.entity(collider2).push_children(&[child]);
    }
}

fn unstick(
    mut commands: Commands,
    context: ResMut<RapierContext>,
    q_children_for_balls_with_children: Query<&Children, With<Ball>>,
    q_rapier_handles_with_bevy_impulse_joints: Query<
        &RapierImpulseJointHandle,
        With<BevyImpulseJoint>,
    >,
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
                    // print!("-");
                    commands
                        .entity(*bevy_impulse_joint_entity)
                        .despawn_recursive();
                }
            }
        }
    }
}

pub struct BallPlugin;

impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(
            NewBallsTimer(Timer::from_seconds(2.0, TimerMode::Repeating)),
            // NewBallsTimer(Timer::from_seconds(1.5, TimerMode::Repeating)),
        )
        .insert_resource(ReproduceBallsTimer(Timer::from_seconds(
            0.025,
            TimerMode::Repeating,
        )))
        .insert_resource(BallAndJointLoopTimer(Timer::from_seconds(
            0.5,
            TimerMode::Repeating,
        )))
        .add_systems(
            Update,
            (
                add_balls,
                contacts,
                unstick,
                reproduce_balls,
                update_life_points,
            ),
        );
    }
}
