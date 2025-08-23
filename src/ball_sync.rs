use bevy::prelude::*;

use crate::ball::{Ball, BallRender};

// Explicitly sync BallRender child transforms from their parent Ball each frame
pub fn sync_ball_render_transforms(
    q_parents: Query<&GlobalTransform, With<Ball>>,
    mut q_children: Query<(&BallRender, &mut Transform)>,
) {
    for (br, mut tf) in q_children.iter_mut() {
        if let Ok(parent_gt) = q_parents.get(br.parent) {
            let p = parent_gt.translation();
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            tf.translation.z = 50.0; // ensure on top
        }
    }
}

