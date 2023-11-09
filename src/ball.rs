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
        Color,
        Commands,
        EventReader,
        Mesh,
        Plugin,
        Res,
        Resource,
        ResMut,
        Time,
        Timer,
        TimerMode,
        Transform,
        // TransformBundle,
        Update,
        Vec2,
        shape::Circle,
    },
    sprite::{
        MaterialMesh2dBundle,
        ColorMaterial,
    }
};
use bevy_rapier2d::prelude::{
    ActiveEvents,
    Collider,
    ContactForceEvent,
    ColliderMassProperties,
    ImpulseJoint,
    RigidBody,
    RevoluteJointBuilder,
    Velocity,
};

use crate::shared_consts::PIXELS_PER_METER;

use crate::setup::{GROUND_HALFWIDTH, WALLS_HALFHEIGHT, GROUND_POSITION};

pub const BALL_RADIUS: f32 = 0.03 * PIXELS_PER_METER;

#[derive(Resource)]
struct NewBallsTimer(pub Timer);


fn add_balls(
    time: Res<Time>,
    mut timer: ResMut<NewBallsTimer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) { 
    // let mut rng = StdRng::seed_from_u64(42);
    let mut rng = thread_rng();

    // update our timer with the time elapsed since the last update
    // if that caused the timer to finish, we say hello to everyone
    if timer.0.tick(time.delta()).just_finished() {
        let transform = Transform::from_xyz(
            rng.gen_range(-1.0 * GROUND_HALFWIDTH, GROUND_HALFWIDTH),
            rng.gen_range(GROUND_POSITION, GROUND_POSITION + (2.0 * WALLS_HALFHEIGHT)),
            0.0,
        );
        commands
            .spawn((
                RigidBody::Dynamic,
                Collider::ball(BALL_RADIUS),
                ColliderMassProperties::Density(0.01),
                // Friction::coefficient(0.7),
                Velocity {
                    linvel: Vec2::new(
                        rng.gen_range(-1.0, 1.0) * PIXELS_PER_METER,
                        rng.gen_range( 1.0, 3.5) * PIXELS_PER_METER,
                    ),
                    angvel: 0.0,
                },
                ActiveEvents::CONTACT_FORCE_EVENTS,
                // Sleeping::disabled(),
                // Ccd::enabled(),
                
                // TransformBundle::from(transform),
                MaterialMesh2dBundle {
                    mesh: meshes.add(Circle::new(BALL_RADIUS).into()).into(),
                    // 4. Put something bright in a dark environment to see the effect
                    material: materials.add(ColorMaterial::from(Color::rgb(7.5, 0.0, 7.5))),
                    transform: transform,
                    ..MaterialMesh2dBundle::default()
                },
            ));
    }
}

fn sticky(
    mut commands: Commands,
    mut contact_force_collisions: EventReader<ContactForceEvent>,
) {
    for ContactForceEvent{collider1, collider2, ..} in contact_force_collisions.iter() {
        let joint = 
            RevoluteJointBuilder::new()
            .local_anchor1(Vec2::new(0.0, 0.0))
            .local_anchor2(Vec2::new(0.0, 0.0))
        ;
        commands.entity(*collider1)
            .insert(ImpulseJoint::new(*collider2, joint));
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
            ),
        );
    }
}