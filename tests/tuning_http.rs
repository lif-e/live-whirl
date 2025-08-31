use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use live_whirl::tuning::{build_router_for_test, PhysicsTuning};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn http_get_and_patch_partial() {
    let (tx, _rx) = std::sync::mpsc::channel::<PhysicsTuning>();
    let mirror = Arc::new(Mutex::new(PhysicsTuning {
        rel_vel_min: 0.15,
        rel_vel_max: 360.0,
        break_force_threshold: 360.0,
        energy_transfer_enabled: true,
        energy_share_diff_threshold: 100,
        energy_share_friendly_rate: 0.5,
        energy_share_parent_not_friendly_child_friendly_rate: 0.75,
        energy_share_parent_friendly_child_not_friendly_rate: 0.25,
        energy_share_hostile_rand_min: 0.5,
        energy_share_hostile_rand_max: 0.9,
        bite_enabled: true,
        bite_size_scale: 1.0,
        genome_bite_size_min: 0,
        genome_bite_size_max: 400,
        genome_energy_share_min: 0.25,
        genome_energy_share_max: 0.75,
        genome_friendly_distance_min: 0.15,
        genome_friendly_distance_max: 1.0,
        genome_friendly_scent_range: 1.0,
        genome_max_age_min: 90,
        genome_max_age_max: 120,
        genome_reproduction_rate_min: 0.011875,
        genome_reproduction_rate_max: 0.0125,
        genome_safe_reproduction_points_min: 0,
        genome_safe_reproduction_points_max: 1000,
        survival_cost_per_tick: 1,
        show_collision_labels: false,
        collision_label_force_min: 2.0,
        show_break_labels: false,
        break_label_impulse_min: 20.0,
        show_age_labels: false,
        age_label_min: 0.0,
        age_label_max: f32::MAX,
        show_energy_labels: false,
        energy_label_min: 0.0,
        energy_label_max: f32::MAX,
    }));

    let app = build_router_for_test(tx, mirror.clone());

    let resp = app.clone().oneshot(Request::builder().uri("/tuning").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let payload = serde_json::json!({
        "stickiness": { "stick_range": { "rel_vel_min": 1.11 } },
        "labels": { "energy": { "show_energy_labels": true } }
    });
    let resp = app.clone().oneshot(Request::builder().method("PATCH").uri("/tuning").header("content-type", "application/json").body(Body::from(payload.to_string())).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let guard = mirror.lock().unwrap();
    assert_eq!(guard.rel_vel_min, 1.11);
    assert!(guard.show_energy_labels);
}

