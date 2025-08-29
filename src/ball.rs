use rand::{rngs::StdRng, Rng};
use std::f32::consts::PI;

use bevy::{
    color::Hsla,
    prelude::{
        App, Assets, Children, Color, Commands, Component,
        Entity, EventReader, GlobalTransform, Plugin, Query, Res, ResMut, Resource, Time, Timer, TimerMode,
        Transform, Update, Vec2, With,
    },
    render::{prelude::Mesh2d},
    render::mesh::Mesh,

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
    markers::{update_force_markers, ForceMarker},
};

pub const BALL_RADIUS: f32 = 0.05 * PIXELS_PER_METER;
const MAX_LIFE_POINTS: u32 = u32::MAX / 2_u32.pow(32 - 10);
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
        (hue_x + hue_y) / 2.0
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
            hue: self.get_hue(),
            saturation: self.get_saturation(),
            lightness: hsla.lightness,
            alpha: hsla.alpha,
        };
        Color::from(new_hsla)
    }
    pub fn get_color(&self) -> Color {
        Color::hsl(self.get_hue(), self.get_saturation(), 0.5)
    }
    fn is_friendly_with(&self, other: Self) -> bool {
        let scent_1 = self.genome_friendly_scent;
        let scent_2 = other.genome_friendly_scent;
        let scent_distance = (scent_1 - scent_2).length();
        scent_distance < self.genome_friendly_distance
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
#[derive(Resource, Default)]
struct FrameCounter{ frame:u64 }

#[derive(Resource, Default)]
struct JointStats{
    created:u64,
    broke_1:u64,
    broke_5:u64,
    broke_30:u64,
}

#[derive(Resource, Default)]
struct CollisionStats {
    samples: u64,
    force_min: f32,
    force_max: f32,
    force_sum: f32,
    rel_min: f32,
    rel_max: f32,
    rel_sum: f32,
    // Simple histograms for approximate percentiles
    force_bins: [u64; 12],
    rel_bins: [u64; 12],
}

impl CollisionStats {
    const FORCE_EDGES: [f32; 12] = [0.1, 0.2, 0.5, 1.0, 2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0, f32::INFINITY];
    const REL_EDGES: [f32; 12] = [0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 12.0, 16.0, f32::INFINITY];

    fn add(&mut self, force_disp: f32, rel_disp: f32) {
        self.samples += 1;
        if self.samples == 1 {
            self.force_min = force_disp;
            self.force_max = force_disp;
            self.rel_min = rel_disp;
            self.rel_max = rel_disp;
        } else {
            self.force_min = self.force_min.min(force_disp);
            self.force_max = self.force_max.max(force_disp);
            self.rel_min = self.rel_min.min(rel_disp);
            self.rel_max = self.rel_max.max(rel_disp);
        }
        self.force_sum += force_disp;
        self.rel_sum += rel_disp;
        // binning
        for (i, edge) in Self::FORCE_EDGES.iter().enumerate() {
            if force_disp <= *edge {
                self.force_bins[i] += 1;
                break;
            }
        }
        for (i, edge) in Self::REL_EDGES.iter().enumerate() {
            if rel_disp <= *edge {
                self.rel_bins[i] += 1;
                break;
            }
        }
    }

    fn percentile_from_bins(bins: &[u64], edges: &[f32], p: f32) -> f32 {
        let total: u64 = bins.iter().sum();
        if total == 0 { return 0.0; }
        let target = (p * total as f32).ceil() as u64;
        let mut acc = 0u64;
        for (i, count) in bins.iter().enumerate() {
            acc += *count;
            if acc >= target {
                return edges[i];
            }
        }
        edges[edges.len() - 1]
    }

    fn snapshot_and_reset(&mut self) -> (u64, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32) {
        let n = self.samples;
        let avg_force = if n > 0 { self.force_sum / n as f32 } else { 0.0 };
        let avg_rel = if n > 0 { self.rel_sum / n as f32 } else { 0.0 };
        let min_f = self.force_min;
        let max_f = self.force_max;
        let min_r = self.rel_min;
        let max_r = self.rel_max;
        let p50_f = Self::percentile_from_bins(&self.force_bins, &Self::FORCE_EDGES, 0.50);
        let p90_f = Self::percentile_from_bins(&self.force_bins, &Self::FORCE_EDGES, 0.90);
        let p99_f = Self::percentile_from_bins(&self.force_bins, &Self::FORCE_EDGES, 0.99);
        let p50_r = Self::percentile_from_bins(&self.rel_bins, &Self::REL_EDGES, 0.50);
        let p90_r = Self::percentile_from_bins(&self.rel_bins, &Self::REL_EDGES, 0.90);
        let p99_r = Self::percentile_from_bins(&self.rel_bins, &Self::REL_EDGES, 0.99);
        // reset
        *self = CollisionStats::default();
        (n, avg_force, min_f, max_f, p50_f, p90_f, p99_f, avg_rel, min_r, max_r, p50_r, p90_r)
    }
}

#[derive(Resource)]
struct CollisionStatsLogTimer(pub Timer);

#[derive(Component)]
struct JointBorn{ frame:u64 }




fn update_life_points(
    mut commands: Commands,
    mut timer: ResMut<BallAndJointLoopTimer>,
    time: Res<Time>,
    mut q_balls_and_colors: Query<(Entity, &mut Ball, &MeshMaterial2d<ColorMaterial>)>,
    q_impulse_joints: Query<(&BevyImpulseJoint, &bevy::prelude::ChildOf)>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut rng_resource: ResMut<RngResource>,
    q_velocities: Query<&Velocity>,
    tuning: Res<crate::tuning::PhysicsTuning>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for (entity, mut ball, color_handle) in q_balls_and_colors.iter_mut() {
        ball.age = if ball.age == u32::MAX { u32::MAX } else { ball.age + 1 };
        if ball.age > ball.genome_max_age {
            ball.life_points = ball.life_points.saturating_sub(tuning.survival_cost_per_tick);
        }
        if ball.life_points <= 9 {
            commands.entity(entity).despawn();
        }
        let Some(color_material) = color_materials.get_mut(color_handle) else { continue };
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
        if tuning.energy_transfer_enabled {
            let parent_points: i32 = parent_ball.life_points as i32;
            let life_points_diff_abs = match parent_points.checked_sub(child_ball.life_points as i32) {
                Some(life_points_diff_abs) => life_points_diff_abs.abs(),
                None => i32::MAX,
            };
            let parent_is_friendly = parent_ball.is_friendly_with(child_ball);
            let child_is_friendly = child_ball.is_friendly_with(parent_ball);
            if parent_is_friendly && child_is_friendly && (life_points_diff_abs as u32 <= tuning.energy_share_diff_threshold) {
                // near-equal friendly: skip transfer
            } else {
                let sharing_rate: f32 = if parent_is_friendly && child_is_friendly {
                    tuning.energy_share_friendly_rate
                } else if !parent_is_friendly && !child_is_friendly {
                    if parent_ball.life_points > child_ball.life_points && (life_points_diff_abs as u32) > tuning.energy_share_diff_threshold {
                        rng.gen_range(tuning.energy_share_hostile_rand_min, tuning.energy_share_hostile_rand_max)
                    } else if parent_ball.life_points < child_ball.life_points && (life_points_diff_abs as u32) > tuning.energy_share_diff_threshold {
                        rng.gen_range(0.1, 0.5)
                    } else {
                        0.5
                    }
                } else if !parent_is_friendly && child_is_friendly {
                    tuning.energy_share_parent_not_friendly_child_friendly_rate
                } else if parent_is_friendly && !child_is_friendly {
                    tuning.energy_share_parent_friendly_child_not_friendly_rate
                } else {
                    0.5
                };
                (parent_ball.life_points, child_ball.life_points) = share_total_roughly(
                    parent_ball.life_points,
                    child_ball.life_points,
                    sharing_rate,
                );
            }
        }

        let Some(parent_color_material) = color_materials.get_mut(parent_color_handle) else { continue };
        parent_color_material.color = parent_ball.transform_color(parent_color_material.color);
        let Some(child_color_material) = color_materials.get_mut(child_color_handle) else { continue };
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
    exclude_entity: Entity,
    x: f32,
    y: f32,
    radius: f32,
    new_ball_radius: f32,
) -> Option<(f32, f32, f32, f32)> {
    let starting_angle = rng.gen_range(0.0, 2.0 * PI);
    let circle_shape = bevy_rapier2d::parry::shape::Ball::new(new_ball_radius);
    for test_angle_ndx in 0..5 {
        let angle = starting_angle + (test_angle_ndx as f32 * (PI / 3.0));
        let total_radius = radius + new_ball_radius;
        let joint_x = x + radius * angle.cos();
        let joint_y = y + radius * angle.sin();
        let new_ball_x = x + total_radius * angle.cos();
        let new_ball_y = y + total_radius * angle.sin();

        // Perform the proximity query, excluding the parent collider
        let mut hit = false;
        let mut filter = QueryFilter::default();
        filter.exclude_collider = Some(exclude_entity);
        rapier_context.intersect_shape(
            Vec2::new(new_ball_x, new_ball_y),
            angle,
            &circle_shape,
            filter,
            |_entity| { hit = true; false }
        );
        if hit { continue; }

        return Some((joint_x, joint_y, new_ball_x, new_ball_y));
    }
    None
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
        Entity,
        &Children,
        &Transform,
        &Collider,
        &MeshMaterial2d<ColorMaterial>,
        &mut Ball,
        &Velocity,
    )>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
    tuning: Res<crate::tuning::PhysicsTuning>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let rng = &mut rng_resource.rng;

    let Ok(ctx) = rapier.single() else { return; };

    for (_parent_entity, children, transform, collider, color_handle, parent_ball, parent_ball_velocity) in
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
            match get_next_ball_position(rng, &ctx, _parent_entity, x, y, radius, new_ball_radius) {
                Some((joint_x, joint_y, new_ball_x, new_ball_y)) => {
                    (joint_x, joint_y, new_ball_x, new_ball_y)
                }
                None => {
                    // Probe failed; count as congestion signal and skip
                    eprintln!("[diag] reproduce probe failed near ({x:.1},{y:.1})");
                    continue;
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
            genome_relative_reproduction_rate: (parent_ball.genome_relative_reproduction_rate + rng.gen_range(-0.01, 0.01)).clamp(tuning.genome_reproduction_rate_min, tuning.genome_reproduction_rate_max),
            genome_bite_size: ((parent_ball.genome_bite_size as f32 + rng.gen_range(-10.0, 10.0)).max(tuning.genome_bite_size_min as f32).min(tuning.genome_bite_size_max as f32) as u32),
            genome_life_points_safe_to_reproduce: ((parent_ball.genome_life_points_safe_to_reproduce as f32 + rng.gen_range(-1000.0, 1000.0)).max(tuning.genome_safe_reproduction_points_min as f32).min(tuning.genome_safe_reproduction_points_max as f32) as u32),
            genome_energy_share_with_children: (parent_ball.genome_energy_share_with_children + rng.gen_range(-0.1, 0.1)).clamp(tuning.genome_energy_share_min, tuning.genome_energy_share_max),
            genome_friendly_scent: Vec2::new(
                parent_ball.genome_friendly_scent.x + rng.gen_range(-0.1, 0.1),
                parent_ball.genome_friendly_scent.y + rng.gen_range(-0.1, 0.1),
            ),
            genome_friendly_distance: (parent_ball.genome_friendly_distance + rng.gen_range(-0.1, 0.1)).clamp(tuning.genome_friendly_distance_min, tuning.genome_friendly_distance_max),
        };

        let parent_color_material = color_materials.get_mut(color_handle).unwrap();
        parent_color_material.color = parent_ball.transform_color(parent_color_material.color);

        // print!(
        //     "\nBaby: Life {: >10}, Max Age {: >10}, Reproduction Rate {: >.4}, Bite Size {: >10}, Safe Reproduction Life {: >10}",
        //     child_ball.life_points,
        //     child_ball.genome_max_age,
        //     child_ball.genome_relative_reproduction_rate,
        //     child_ball.genome_bite_size,
        //     child_ball.genome_life_points_safe_to_reproduce,
        // );

        eprintln!("[diag] reproduce spawn at ({:.1},{:.1})", new_ball_x, new_ball_y);

        // Spawn physics entity with render components combined (no BallRender child)
        let initial = child_ball.get_color();
        let _entity = commands
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
                Restitution::new(0.1),
                Transform::from_xyz(new_ball_x, new_ball_y, 0.0),
                GlobalTransform::default(),
                Mesh2d(_mesh_assets.ball_circle.clone()),
                MeshMaterial2d(color_materials.add(ColorMaterial::from(initial))),
            ))
            .id();
    }
}

#[derive(Resource)]
struct NewBallsTimer(pub Timer);

struct Box2D {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}
const SPAWN_BOX: Box2D = Box2D {
    min_x: (-0.5 * GROUND_WIDTH) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    max_x: (0.5 * GROUND_WIDTH) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
    // min_y: (-0.5 *   WALL_HEIGHT) + (BALL_RADIUS * 2.0) + (0.5 * WALL_THICKNESS),
    min_y: (0.5 * WALL_HEIGHT) - (BALL_RADIUS * 4.0) - (0.5 * WALL_THICKNESS),
    max_y: (0.5 * WALL_HEIGHT) - (BALL_RADIUS * 2.0) - (0.5 * WALL_THICKNESS),
};

const MIN_LINEAR_VELOCITY: Vec2 = Vec2::new(-1.0, -1.0);
const MAX_LINEAR_VELOCITY: Vec2 = Vec2::new(1.0, 1.0);

// Legacy constants retained for reference; live values come from PhysicsTuning
const STICKY_MIN_FORCE: f32 = 0.05;
const STICKY_CREATION_FORCE_MAX: f32 = 20.0;
const STICKY_BREAKING_FORCE: f32 = 20.0;

fn add_balls(
    time: Res<Time>,
    mut timer: ResMut<NewBallsTimer>,
    mut commands: Commands,
    mut rng_resource: ResMut<RngResource>,
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
    mesh_assets: Res<crate::setup::MeshAssets2d>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    _q_balls: Query<Entity, With<Ball>>,
    tuning: Res<crate::tuning::PhysicsTuning>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let rng = &mut rng_resource.rng;
    let linearvelocity: Vec2 = Vec2::new(
        rng.gen_range(MIN_LINEAR_VELOCITY.x, MAX_LINEAR_VELOCITY.x),
        rng.gen_range(MIN_LINEAR_VELOCITY.y, MAX_LINEAR_VELOCITY.y),
    );

    let t = tuning.into_inner();
    let scent_r = t.genome_friendly_scent_range;
    let ball = Ball {
        age: 0,
        life_points: MAX_LIFE_POINTS,
        genome_max_age: rng.gen_range(t.genome_max_age_min, t.genome_max_age_max),
        genome_relative_reproduction_rate: rng.gen_range(t.genome_reproduction_rate_min, t.genome_reproduction_rate_max),
        genome_bite_size: rng.gen_range(t.genome_bite_size_min, t.genome_bite_size_max),
        genome_life_points_safe_to_reproduce: rng.gen_range(t.genome_safe_reproduction_points_min, t.genome_safe_reproduction_points_max),
        genome_energy_share_with_children: rng.gen_range(t.genome_energy_share_min, t.genome_energy_share_max),
        genome_friendly_scent: Vec2::new(rng.gen_range(-scent_r, scent_r), rng.gen_range(-scent_r, scent_r)),
        genome_friendly_distance: rng.gen_range(t.genome_friendly_distance_min, t.genome_friendly_distance_max),
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
    // Spawn single entity with both physics and render components
    let initial = ball.get_color();
    let _entity = commands
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
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::default(),
            Mesh2d(mesh_assets.ball_circle.clone()),
            MeshMaterial2d(materials.add(ColorMaterial::from(initial))),
        ))
        .id();
}

const MAX_JOINTS: usize = 10;
const PAIRWISE_JOINTS_ALLOWED: u8 = 2;
const JOINT_DISTANCE: f32 = BALL_RADIUS * 0.03;


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
    mut meshes: ResMut<Assets<Mesh>>,
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
    // Obtain context once; if absent, bail early
    mut contact_force_collisions: EventReader<ContactForceEvent>,

    mut q_balls: Query<&mut Ball>,
    q_children_for_balls: Query<&Children, With<Ball>>,
    q_bevy_impulse_joints: Query<&BevyImpulseJoint>,
    q_velocities: Query<&Velocity>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    q_color_material_handles: Query<&MeshMaterial2d<ColorMaterial>>,
    q_global_transforms: Query<&GlobalTransform>,
    q_is_ball: Query<(), With<Ball>>,
    q_existing_markers: Query<(&Transform, &ForceMarker)>,
    frame_counter: ResMut<FrameCounter>,
    mut joint_stats: ResMut<JointStats>,
    tuning: Res<crate::tuning::PhysicsTuning>,
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
        let force = *total_force_magnitude;


        // Record stats before further filtering
        let rel_speed = if let Ok([v1, v2]) = q_velocities.get_many([collider1, collider2]) {
            (v1.linvel - v2.linvel).length()
        } else { 0.0 };
        // stats collection removed to reduce system params

        // Filter: only show markers for ball-to-ball collisions
        let is_ball1 = q_is_ball.get(collider1).is_ok();
        let is_ball2 = q_is_ball.get(collider2).is_ok();
        if is_ball1 && is_ball2 && !already_has_max_pairwise_joints(&q_children_for_balls, &q_bevy_impulse_joints, &collider1, &collider2) {
            // Skip negligible contacts: very low relative speed OR very small force
            let mut negligible = false;
            if let Ok([v1, v2]) = q_velocities.get_many([collider1, collider2]) {
                let rel = v1.linvel - v2.linvel;
                // Use live tuning value for negligible filtering
                if rel.length() < tuning.rel_vel_min { negligible = true; }
            }
            // Only relative-velocity based negligible filter
            if negligible {
                // Show white collision label for non-sticking collision (neg result)
                if tuning.show_collision_labels {
                    let display_force = force / PIXELS_PER_METER;
                    if display_force >= tuning.collision_label_force_min {
                        if let (Ok(tf1), Ok(tf2)) = (q_global_transforms.get(collider1), q_global_transforms.get(collider2)) {
                            let mid = (tf1.translation().truncate() + tf2.translation().truncate()) * 0.5;
                            let epsilon_x = 50.0; // pixels
                            let mut max_stack: u32 = 0;
                            for (tf, _) in q_existing_markers.iter() {
                                let dx = (tf.translation.x - mid.x).abs();
                                if dx < epsilon_x {
                                    let dy = (tf.translation.y - mid.y).max(0.0);
                                    let line_sep = 1.2 * (2.0 * BALL_RADIUS);
                                    let approx_stack = (dy / line_sep).floor() as u32;
                                    if approx_stack > max_stack { max_stack = approx_stack; }
                                }
                            }
                            let stack_lines = max_stack + 1;
                            crate::markers::spawn_force_marker(&mut commands, &mut meshes, &mut color_materials, mid, format!("{:.1}", display_force), Color::srgba(1.0, 1.0, 1.0, 1.0), stack_lines);
                        }
                    }
                }
                continue;
            }

            // Green labels are only spawned after a successful joint creation below
        }

        if force > tuning.break_force_threshold {
            // Mutably access both balls so changes persist
            let [mut b1, mut b2] = match q_balls.get_many_mut([collider1, collider2]) {
                Ok(bs) => bs,
                Err(_) => continue,
            };
            let [v1, v2] = match q_velocities.get_many([collider1, collider2]) {
                Ok(velocities) => velocities,
                Err(_) => continue,
            };

            let scent_distance = (b1.genome_friendly_scent - b2.genome_friendly_scent).length();
            let one_is_friendly = scent_distance < b1.genome_friendly_distance;
            let two_is_friendly = scent_distance < b2.genome_friendly_distance;

            if !(one_is_friendly && two_is_friendly) {
                if !one_is_friendly && (v1.linvel.length().abs() > v2.linvel.length().abs()) {
                    let bite_size = b1.genome_bite_size;
                    b2.life_points = b2.life_points.saturating_sub(bite_size);
                    b1.life_points = b1.life_points.saturating_add(bite_size);
                } else if !two_is_friendly && (v2.linvel.length().abs() > v1.linvel.length().abs()) {
                    let bite_size = b2.genome_bite_size;
                    b1.life_points = b1.life_points.saturating_sub(bite_size);
                    b2.life_points = b2.life_points.saturating_add(bite_size);
                }

                // Update visible colors by walking to BallRender child to find the material handle
                let mut update_child_color = |rb_entity: Entity, ball: &Ball| {
                    if let Ok(children) = q_children_for_balls.get(rb_entity) {
                        for &c in children.iter() {
                            if let Ok(handle) = q_color_material_handles.get(c) {
                                if let Some(mat) = color_materials.get_mut(handle) {
                                    mat.color = ball.transform_color(mat.color);
                                }
                                break;
                            }
                        }
                    }
                };
                update_child_color(collider1, &b1);
                update_child_color(collider2, &b2);
            }
            // Do not continue here; allow joint creation logic to run below as well
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


        // Relative velocity gate for joint creation
        let mut rel_ok = false;
        let rel_len = if let Ok([v1, v2]) = q_velocities.get_many([collider1, collider2]) {
            let r = (v1.linvel - v2.linvel).length();
            rel_ok = r >= tuning.rel_vel_min && r <= tuning.rel_vel_max;
            r
        } else { 0.0 };
        if !rel_ok {
            // Non-sticking collision (white label if above threshold)
            if tuning.show_collision_labels {
                let display_force = force / PIXELS_PER_METER;
                if display_force >= tuning.collision_label_force_min {
                    if let (Ok(tf1), Ok(tf2)) = (q_global_transforms.get(collider1), q_global_transforms.get(collider2)) {
                        let mid = (tf1.translation().truncate() + tf2.translation().truncate()) * 0.5;
                        let epsilon_x = 50.0; // pixels
                        let mut max_stack: u32 = 0;
                        for (tf, _) in q_existing_markers.iter() {
                            let dx = (tf.translation.x - mid.x).abs();
                            if dx < epsilon_x {
                                let dy = (tf.translation.y - mid.y).max(0.0);
                                let line_sep = 1.2 * (2.0 * BALL_RADIUS);
                                let approx_stack = (dy / line_sep).floor() as u32;
                                if approx_stack > max_stack { max_stack = approx_stack; }
                            }
                        }
                        let stack_lines = max_stack + 1;
                        crate::markers::spawn_force_marker(&mut commands, &mut meshes, &mut color_materials, mid, format!("{:.1}", display_force), Color::srgba(1.0, 1.0, 1.0, 1.0), stack_lines);
                    }
                }
            }
            continue;
        }

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

        // Project anchors along the normal but clamp near the contact to reduce tension
        let n1 = contact_point.local_p1().normalize_or_zero();
        let n2 = contact_point.local_p2().normalize_or_zero();
        let e1_sticky_point: Vec2 = n1 * (BALL_RADIUS + JOINT_DISTANCE * 0.5);
        let e2_sticky_point: Vec2 = n2 * (BALL_RADIUS + JOINT_DISTANCE * 0.5);
        // Create a dedicated joint entity so our children-based caps/queries see it
        let joint_entity = commands
            .spawn((
                BevyImpulseJoint::new(
                    collider1,
                    RevoluteJointBuilder::new()
                        .local_anchor1(e1_sticky_point)
                        .local_anchor2(e2_sticky_point)
                        .build(),
                ),
                JointBorn { frame: frame_counter.frame },
            ))
            .id();
        joint_stats.created += 1;
        commands.entity(collider2).add_child(joint_entity);
        eprintln!("[diag] joint_create ok between {:?} and {:?} (child {:?})", collider1, collider2, joint_entity);
        // Green label for successful stick
        if tuning.show_collision_labels {
            let display_force = force / PIXELS_PER_METER;
            if display_force >= tuning.collision_label_force_min {
                if let (Ok(tf1), Ok(tf2)) = (q_global_transforms.get(collider1), q_global_transforms.get(collider2)) {
                    let mid = (tf1.translation().truncate() + tf2.translation().truncate()) * 0.5;
                    let epsilon_x = 50.0; // pixels
                    let mut max_stack: u32 = 0;
                    for (tf, _) in q_existing_markers.iter() {
                        let dx = (tf.translation.x - mid.x).abs();
                        if dx < epsilon_x {
                            let dy = (tf.translation.y - mid.y).max(0.0);
                            let line_sep = 1.2 * (2.0 * BALL_RADIUS);
                            let approx_stack = (dy / line_sep).floor() as u32;
                            if approx_stack > max_stack { max_stack = approx_stack; }
                        }
                    }
                    let stack_lines = max_stack + 1;
                    crate::markers::spawn_force_marker(&mut commands, &mut meshes, &mut color_materials, mid, format!("{:.1}", display_force), Color::srgba(0.2, 1.0, 0.2, 1.0), stack_lines);
                }
            }
        }
    }
}


fn unstick(
    mut commands: Commands,
    rapier: bevy_rapier2d::prelude::ReadRapierContext,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,

    q_balls_with_children: Query<(Entity, &Children), With<Ball>>,
    q_rapier_handles_with_bevy_impulse_joints: Query<
        &RapierImpulseJointHandle,
        With<BevyImpulseJoint>,
    >,
    q_joint_born: Query<&JointBorn>,
    q_global_transforms: Query<&GlobalTransform>,
    q_existing_markers: Query<(&Transform, &ForceMarker)>,
    frame_counter: ResMut<FrameCounter>,

    mut joint_stats: ResMut<JointStats>,
    tuning: Res<crate::tuning::PhysicsTuning>,

) {
    for (_ball_entity, children) in q_balls_with_children.iter() {

        for child_entity in children.iter() {
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
                if impulse_magnitude > tuning.break_force_threshold {
                    eprintln!("[diag] joint_break impulse={impulse_magnitude:.6}");
                    if let Ok(born) = q_joint_born.get(*bevy_impulse_joint_entity) {
                        let age = frame_counter.frame.saturating_sub(born.frame);
                        if age <= 1 { joint_stats.broke_1 += 1; }
                        if age <= 5 { joint_stats.broke_5 += 1; }
                        if age <= 30 { joint_stats.broke_30 += 1; }
                    }
                    if tuning.show_break_labels && impulse_magnitude >= tuning.break_label_impulse_min {
                        // Spawn red marker at the parent ball's transform (joint entity has no Transform)
                        // Stack above nearby markers at the parent ball's position
                        let pos = if let Ok(ball_tf) = q_global_transforms.get(_ball_entity) {
                            Vec2::new(ball_tf.translation().x, ball_tf.translation().y)
                        } else {
                            Vec2::ZERO
                        };
                        let epsilon_x = 50.0;
                        let mut max_stack: u32 = 0;
                        for (tf, _) in q_existing_markers.iter() {
                            let dx = (tf.translation.x - pos.x).abs();
                            if dx < epsilon_x {
                                let dy = (tf.translation.y - pos.y).max(0.0);
                                let line_sep = 1.2 * (2.0 * BALL_RADIUS);
                                let approx_stack = (dy / line_sep).floor() as u32;
                                if approx_stack > max_stack { max_stack = approx_stack; }
                            }
                        }
                        let stack_lines = max_stack + 1;
                        crate::markers::spawn_force_marker(&mut commands, &mut meshes, &mut materials, pos, format!("{:.1}", impulse_magnitude), Color::srgba(1.0, 0.2, 0.2, 1.0), stack_lines);
                    }

                    commands
                        .entity(*bevy_impulse_joint_entity)
                        .despawn();
                }
            }
        }
    }
}

pub struct BallPlugin;

impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NewBallsTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
            .insert_resource(ReproduceBallsTimer(Timer::from_seconds(0.025, TimerMode::Repeating)))
            .insert_resource(BallAndJointLoopTimer(Timer::from_seconds(0.5, TimerMode::Repeating)))
            .insert_resource(FrameCounter::default())
            .insert_resource(JointStats::default())
            .insert_resource(CollisionStats::default())
            .insert_resource(CollisionStatsLogTimer(Timer::from_seconds(1.0, TimerMode::Repeating)))
            .add_systems(Update, (add_balls, reproduce_balls))
            .add_systems(Update, contacts)
            .add_systems(Update, unstick)
            .add_systems(Update, update_force_markers)
            .add_systems(Update, collision_stats_logger)
            .add_systems(Update, update_life_points);
    }
}

fn collision_stats_logger(
    time: Res<Time>,
    mut timer: ResMut<CollisionStatsLogTimer>,
    mut stats: ResMut<CollisionStats>,
) {
    if !timer.0.tick(time.delta()).just_finished() { return; }
    let (n, avg_f, min_f, max_f, p50_f, p90_f, p99_f, avg_r, min_r, max_r, p50_r, p90_r) = stats.snapshot_and_reset();
    if n > 0 {
        eprintln!("[diag] collisions stats: n={n} force avg={avg_f:.2} min={min_f:.2} max={max_f:.2} p50={p50_f:.2} p90={p90_f:.2} p99={p99_f:.2} | rel avg={avg_r:.2} min={min_r:.2} max={max_r:.2} p50={p50_r:.2} p90={p90_r:.2}");
    }
}


