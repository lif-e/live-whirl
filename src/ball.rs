use rand::{rngs::StdRng, Rng};
use std::f32::consts::PI;

use bevy::{
    color::Hsla,
    prelude::{
        App, Assets, Children, Color, Commands, Component,
        Entity, EventReader, GlobalTransform, Local, Plugin, Query, Res, ResMut, Resource, Time, Timer, TimerMode,
        Transform, Update, Vec2, With,
    },
    render::{prelude::Mesh2d, view::RenderLayers},
    sprite::{ColorMaterial, MeshMaterial2d},
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

// Render-only companion for a Ball entity (kept separate from physics parent)
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct BallRender {
    pub parent: Entity,
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
        let s = ((self.life_points as f32 / MAX_LIFE_POINTS as f32)
            * COLOR_SATURATION_SCALE_FACTOR)
            + COLOR_SATURATION_MINIMUM;
        s.clamp(COLOR_SATURATION_MINIMUM, 1.0)
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
    mut q_balls_and_colors: Query<(Entity, &mut Ball, &MeshMaterial2d<ColorMaterial>)>,
    q_impulse_joints: Query<(&BevyImpulseJoint, &bevy::prelude::ChildOf)>,
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
            commands.entity(entity).despawn();
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
            match q_balls_and_colors.get_many_mut([parent.parent(), joint.parent]) {
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
            } else if parent_ball.life_points < child_ball.life_points && life_points_diff_abs > 100
            {
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
    rapier_context: &RapierContext,
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

        let circle_shape = bevy_rapier2d::parry::shape::Ball::new(new_ball_radius);

        // Perform the proximity query
        let mut hit = false;
        rapier_context.intersect_shape(
            Vec2::new(new_ball_x, new_ball_y),
            angle,
            &circle_shape,
            QueryFilter::default(),
            |_entity| { hit = true; false }
        );
        if hit { continue; }

        return Some((joint_x, joint_y, new_ball_x, new_ball_y));
    }
    return None;
}

fn reproduce_balls(
    mut commands: Commands,
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
    time: Res<Time>,
    mut timer: ResMut<ReproduceBallsTimer>,
    mut rng_resource: ResMut<RngResource>,

    _mesh_assets: Res<crate::setup::MeshAssets2d>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    q_children_and_transform_and_collider_and_color_handles_with_balls: Query<(
        &Children,
        &Transform,
        &Collider,
        &MeshMaterial2d<ColorMaterial>,
        &mut Ball,
        &Velocity,
    )>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let rng = &mut rng_resource.rng;

    let Ok(ctx) = rapier.single() else { return; };

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
            match get_next_ball_position(rng, &ctx, x, y, radius, new_ball_radius) {
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
        // Force bright green for debugging visibility parity with test ball
        // TEMP DEBUG: neon magenta to maximize visibility
        let _child_color_material = ColorMaterial::from(Color::hsl(300.0, 1.0, 0.5));

        // print!(
        //     "\nBaby: Life {: >10}, Max Age {: >10}, Reproduction Rate {: >.4}, Bite Size {: >10}, Safe Reproduction Life {: >10}",
        //     child_ball.life_points,
        //     child_ball.genome_max_age,
        //     child_ball.genome_relative_reproduction_rate,
        //     child_ball.genome_bite_size,
        //     child_ball.genome_life_points_safe_to_reproduce,
        // );

        eprintln!("[diag] reproduce spawn at ({:.1},{:.1})", new_ball_x, new_ball_y);

        // Spawn physics parent and render child separately
        let parent = commands
            .spawn((
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
                // Ensure parent participates in visibility hierarchy
                bevy::render::view::Visibility::Visible,
                bevy::render::view::InheritedVisibility::VISIBLE,
                Transform::from_xyz(new_ball_x, new_ball_y, 0.0),
                GlobalTransform::default(),
                RenderLayers::layer(1),
            ))
            .id();
        let child = commands
            .spawn((
                bevy::sprite::Sprite::from_color(Color::hsl(0.0, 1.0, 0.6), Vec2::new(36.0, 36.0)),
                bevy::render::view::Visibility::Visible,
                bevy::prelude::InheritedVisibility::VISIBLE,
                Transform::from_xyz(0.0, 0.0, 30.0),
                bevy::render::view::NoFrustumCulling,
                RenderLayers::from_layers(&[0, 1]),
                BallRender { parent },
            ))
            .id();
        commands.entity(parent).add_child(child);
    }
}

#[derive(Resource)]
struct NewBallsTimer(pub Timer);

struct Box2D {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    min_z: f32,
    max_z: f32,
}
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
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
    mesh_assets: Res<crate::setup::MeshAssets2d>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    _q_balls: Query<Entity, With<Ball>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let rng = &mut rng_resource.rng;
    let linearvelocity: Vec2 = Vec2::new(
        rng.gen_range(MIN_LINEAR_VELOCITY.x, MAX_LINEAR_VELOCITY.x),
        rng.gen_range(MIN_LINEAR_VELOCITY.y, MAX_LINEAR_VELOCITY.y),
    );

    let ball = Ball {
        age: 0,
        life_points: MAX_LIFE_POINTS,
        genome_max_age: rng.gen_range(90, 120),
        genome_relative_reproduction_rate: rng.gen_range(0.00625 * 1.9, 0.00625 * 2.0),
        genome_bite_size: rng.gen_range(0, 400),
        genome_life_points_safe_to_reproduce: rng.gen_range(0, 1000),
        genome_energy_share_with_children: rng.gen_range(0.25, 0.75),
        genome_friendly_scent: Vec2::new(rng.gen_range(-1.0, 1.0), rng.gen_range(-1.0, 1.0)),
        genome_friendly_distance: rng.gen_range(0.15, 1.0),
    };
    // Force bright green for debugging visibility parity with test ball
    // TEMP DEBUG: neon magenta to maximize visibility
    let _color_material: ColorMaterial = ColorMaterial::from(Color::hsl(300.0, 1.0, 0.5));

    // Collider for the spawned rigid body
    let circle_collider = Collider::ball(BALL_RADIUS);

    let Ok(ctx) = rapier.single() else { return; };

    // Spawn in a mid band that is clearly within the playfield
    let x = rng.gen_range(SPAWN_BOX.min_x, SPAWN_BOX.max_x);
    let y = rng.gen_range(0.20 * WALL_HEIGHT, 0.35 * WALL_HEIGHT);
    eprintln!("[diag] add_balls spawn at ({:.1},{:.1})", x, y);

    // Perform the proximity query using a shape (not a Collider component)
    let query_shape = bevy_rapier2d::parry::shape::Ball::new(BALL_RADIUS);
    let mut hit = false;
    ctx.intersect_shape(
        Vec2::new(x, y),
        0.0,
        &query_shape,
        QueryFilter::default(),
        |_entity| { hit = true; false }
    );
    if hit { return; }

    // update our timer with the time elapsed since the last update
    // if that caused the timer to finish, we say hello to everyone
    // Spawn physics parent (no render) and a render child (pure sprite)
    let _parent = commands
        .spawn((
            ball,
            RigidBody::Dynamic,
            circle_collider,
            ColliderMassProperties::Density(0.001),
            Friction::coefficient(0.7),
            Velocity {
                linvel: linearvelocity * PIXELS_PER_METER,
                angvel: 0.0,
            },
            ActiveEvents::CONTACT_FORCE_EVENTS,
            Restitution::new(0.1),
            // Ensure visibility hierarchy is established for the render child
            bevy::render::view::Visibility::Visible,
            bevy::render::view::InheritedVisibility::VISIBLE,
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::default(),
            RenderLayers::layer(1),
        ))
        .id();
    // Use a smaller bright sprite to reduce occlusion risk
    let child = commands
        .spawn((
            Mesh2d(mesh_assets.ball_circle.clone()),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::hsl(300.0, 1.0, 0.5)))),
            bevy::render::view::Visibility::Visible,
            bevy::prelude::InheritedVisibility::VISIBLE,
            Transform::from_xyz(0.0, 0.0, 50.0),
            bevy::render::view::NoFrustumCulling,
            RenderLayers::from_layers(&[0, 1]),
            BallRender { parent: _parent },
        ))
        .id();
    eprintln!("[diag] spawned BallRender child {child:?} for parent {_parent:?} at ({x:.1},{y:.1})");
    commands.entity(_parent).add_child(child);
    // TEMP DEBUG: Also spawn a bright square sprite marker co-parented to move with the ball
    let marker = commands
        .spawn((
            bevy::sprite::Sprite::from_color(Color::hsl(60.0, 1.0, 0.6), Vec2::new(30.0, 30.0)),
            bevy::render::view::Visibility::Visible,
            bevy::render::view::InheritedVisibility::VISIBLE,
            Transform::from_xyz(0.0, 0.0, 30.0),
            bevy::render::view::NoFrustumCulling,
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(_parent).add_child(marker);
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
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
    // Obtain context once; if absent, bail early
    mut contact_force_collisions: EventReader<ContactForceEvent>,
    q_balls: Query<&mut Ball>,
    q_children_for_balls: Query<&Children, With<Ball>>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
    q_velocities: Query<&Velocity>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    q_color_material_handles: Query<&MeshMaterial2d<ColorMaterial>>,
) {
    let Ok(ctx) = rapier.single() else { return; };

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
        let contact_pair = match ctx.contact_pair(collider1, collider2) {
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

        commands.entity(collider2).add_child(child);
    }
}

fn unstick(
    mut commands: Commands,
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
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
            let Ok(ctx) = rapier.single() else { continue; };
            let rapier_joint = match ctx.joints.impulse_joints.get(rapier_handle.0) {
                Some(rapier_joint) => rapier_joint,
                None => continue,
            };
            for impulse in rapier_joint.impulses.column_iter() {
                let impulse_magnitude: f32 = Vec2::new(impulse.x, impulse.y).length();
                if impulse_magnitude > STICKY_BREAKING_FORCE {
                    // print!("-");
                    commands
                        .entity(*bevy_impulse_joint_entity)
                        .despawn();
                }
            }
        }
    }
}

pub struct BallPlugin;

// Diagnostic: spawn a simple, no-physics ball via BallPlugin to test offscreen visibility
fn spawn_debug_simple_ball(
    mut commands: Commands,
    headless: Option<Res<crate::setup::Headless>>,
    video_req: Option<Res<crate::setup::VideoExportRequest>>,
    mut spawned: Local<bool>,
    mesh_assets: Option<Res<crate::setup::MeshAssets2d>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if *spawned { return; }
    let is_headless = headless.map(|h| h.0).unwrap_or(false);
    let has_video = video_req.is_some();
    let Some(mesh_assets) = mesh_assets else { return; };
    if !(is_headless && has_video) { return; }

    let x = -200.0;
    let y = 1800.0;
    let material = materials.add(ColorMaterial::from(Color::hsl(120.0, 1.0, 0.5)));
    commands.spawn((
        Mesh2d(mesh_assets.ball_circle.clone()),
        MeshMaterial2d(material),
        bevy::render::view::Visibility::Visible,
        bevy::render::view::InheritedVisibility::VISIBLE,
        Transform::from_xyz(x, y, 2.0),
        RenderLayers::layer(1),
    ));
    eprintln!("[diag] spawned debug simple ball at ({:.1},{:.1})", x, y);
    *spawned = true;
}

impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(
            NewBallsTimer(Timer::from_seconds(2.0, TimerMode::Repeating)),
        )
        .insert_resource(ReproduceBallsTimer(Timer::from_seconds(
            0.025,
            TimerMode::Repeating,
        )))
        .insert_resource(BallAndJointLoopTimer(Timer::from_seconds(0.5, TimerMode::Repeating)))
        // Periodic debug logging of ball count (every ~2s)
        .add_systems(Update, (ball_count_logger))
        // Also log BallRender companions
        .add_systems(Update, ball_render_logger)
        // TEMP DEBUG: mirror the first Ball as a bright Sprite to eliminate Mesh2d/material as a factor
        .add_systems(Update, update_ball_debug_mirror)
        // Ensure spawn -> transforms/visibility -> extract ordering
        // Move to Update so the render extract sees them earlier this frame
        .add_systems(Update, (add_balls, reproduce_balls))
        .add_systems(Update, contacts)
        .add_systems(Update, unstick)
        .add_systems(Update, update_life_points)
        // Diagnostic: spawn a simple ball via the same plugin
        .add_systems(Update, spawn_debug_simple_ball);
    }
}

#[derive(Component)]
struct BallDebugMirror;

// Spawns a single bright Sprite that tracks the first Ball's position.
// This bypasses Mesh2d/Material2d, parenting, and z-order complexities.
fn update_ball_debug_mirror(
    mut commands: Commands,
    mut marker: Local<Option<Entity>>,
    q_balls: Query<&GlobalTransform, With<Ball>>,
    mut q_marker_tf: Query<&mut Transform, With<BallDebugMirror>>,
) {
    // Track the first ball if any
    let first_ball = q_balls.iter().next().copied();
    match (*marker, first_ball) {
        (None, Some(gt)) => {
            let pos = gt.translation();
            let e = commands
                .spawn((
                    bevy::sprite::Sprite::from_color(Color::hsl(60.0, 1.0, 0.6), Vec2::new(60.0, 60.0)),
                    bevy::render::view::Visibility::Visible,
                    bevy::render::view::visibility::InheritedVisibility::VISIBLE,
                    // Put well above pegs/walls
                    Transform::from_xyz(pos.x, pos.y, 40.0),
                    bevy::render::view::NoFrustumCulling,
                    RenderLayers::layer(1),
                    BallDebugMirror,
                ))
                .id();
            *marker = Some(e);
        }
        (Some(e), Some(gt)) => {
            if let Ok(mut tf) = q_marker_tf.get_mut(e) {
                let p = gt.translation();

                tf.translation.x = p.x;
                tf.translation.y = p.y;
                tf.translation.z = 40.0;
            }
        }
        // No balls yet: do nothing
        _ => {}
    }
}

fn ball_render_logger(
    q: Query<(Entity, &GlobalTransform, Option<&bevy::render::view::visibility::ViewVisibility>, Option<&bevy::render::view::visibility::InheritedVisibility>), With<BallRender>>,
    time: Res<Time>,
) {
    static mut ACC: f32 = 0.0;
    let dt = time.delta_secs();
    unsafe {
        ACC += dt;
        if ACC >= 1.0 {
            let count = q.iter().count();
            let mut it = q.iter();
            let sample = it.next().map(|(_e, g, vv, iv)| {
                let t = g.translation();
                let vv = vv.map(|v| v.get()).unwrap_or(false);
                let iv = iv.map(|v| v.get()).unwrap_or(true);
                ((t.x, t.y, t.z), vv, iv)
            });
            eprintln!("[diag] ball_render: count={count} sample={sample:?}");
            ACC = 0.0;
        }
    }
}


fn ball_count_logger(q: Query<Entity, With<Ball>>, time: Res<Time>) {
    static mut ACCUM: f32 = 0.0;
    let dt = time.delta_secs();
    unsafe {
        ACCUM += dt;
        if ACCUM >= 2.0 {
            eprintln!("[diag] balls={}", q.iter().count());
            ACCUM = 0.0;
        }
    }
}


