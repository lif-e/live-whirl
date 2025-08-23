use bevy::prelude::*;

#[derive(Component)]
pub struct OffscreenCam;

#[derive(Resource, Default, Clone, Copy)]
pub struct OrthoScale(pub f32);

pub fn set_ortho_scale_after_spawn(
    scale: Option<Res<OrthoScale>>,
    mut q: Query<&mut bevy::render::camera::Projection, With<OffscreenCam>>,
) {
    let desired = scale.map(|s| s.0).unwrap_or(1.0);
    for mut proj in q.iter_mut() {
        if let bevy::render::camera::Projection::Orthographic(ref mut ortho) = *proj {
            ortho.scale = desired;
        }
    }
}

