use bevy::prelude::*;
use bevy::render::prelude::Mesh2d;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use crate::ball::BALL_RADIUS;

#[derive(Component)]
pub struct ForceMarker { pub elapsed: f32 }

/// Spawn a force marker at `pos`, shifted up by 5 ball diameters plus `stack_lines` extra line steps.
pub fn spawn_force_marker(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    pos: Vec2,
    text: String,
    color: Color,
    stack_lines: u32,
) {
    let base_y_offset = 5.0 * (2.0 * BALL_RADIUS);
    let line_sep = 1.2 * (2.0 * BALL_RADIUS);
    let y_offset = base_y_offset + (stack_lines as f32) * line_sep;

    // Foreground text (parent)
    let text_entity = commands
        .spawn((
            Text2d::new(text),
            TextColor(color),
            TextFont::from_font(Default::default()).with_font_size(96.0),
            Transform::from_xyz(pos.x, pos.y + y_offset, 1000.0),
            ForceMarker { elapsed: 0.0 },
        ))
        .id();

    // Backing rectangle (child) to improve readability under overlap
    let bg_size = Vec2::new(10.0 * BALL_RADIUS, 3.0 * BALL_RADIUS);
    let bg_color = Color::srgba(0.0, 0.0, 0.0, 0.6);
    commands.entity(text_entity).with_children(|p|{
        p.spawn((
            Mesh2d(meshes.add(bevy::math::primitives::Rectangle::from_size(bg_size))),
            MeshMaterial2d(materials.add(ColorMaterial::from(bg_color))),
            Transform::from_xyz(0.0, 0.0, -0.5),
        ));
    });
}

/// Shorter lifetime: full alpha for 0.5s, fade out by 2.0s, then despawn.
pub fn update_force_markers(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ForceMarker, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (e, mut marker, mut color) in q.iter_mut() {
        marker.elapsed += dt;
        let t = marker.elapsed;
        if t <= 0.5 {
            let srgb = color.0.to_srgba();
            color.0 = Color::srgba(srgb.red, srgb.green, srgb.blue, 1.0);
        } else if t <= 2.0 {
            let a = 1.0 - ((t - 0.5) / 1.5);
            let srgb = color.0.to_srgba();
            color.0 = Color::srgba(srgb.red, srgb.green, srgb.blue, a.clamp(0.0, 1.0));
        } else {
            commands.entity(e).despawn();
        }
    }
}

