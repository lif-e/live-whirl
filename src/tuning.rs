use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use axum::{extract::State, routing::{get, patch}, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder;

use bevy::prelude::{Resource, Res, ResMut, NonSend};

#[derive(Debug, Clone, Serialize, Deserialize, Default, Resource)]
pub struct PhysicsTuning {
    pub rel_vel_min: f32,
    pub rel_vel_max: f32,
    // Breaking threshold (raw Rapier impulse units for joints)
    pub break_force_threshold: f32,
    // Energy transfer and bite behavior between stuck pairs
    pub energy_transfer_enabled: bool,
    pub energy_share_diff_threshold: u32,
    pub energy_share_friendly_rate: f32,
    pub energy_share_parent_not_friendly_child_friendly_rate: f32,
    pub energy_share_parent_friendly_child_not_friendly_rate: f32,
    pub energy_share_hostile_rand_min: f32,
    pub energy_share_hostile_rand_max: f32,
    pub bite_enabled: bool,
    pub bite_size_scale: f32,
    // Genome generation ranges for new balls
    pub genome_bite_size_min: u32,
    pub genome_bite_size_max: u32,
    pub genome_energy_share_min: f32,
    pub genome_energy_share_max: f32,
    pub genome_friendly_distance_min: f32,
    pub genome_friendly_distance_max: f32,
    pub genome_friendly_scent_range: f32,
    pub genome_max_age_min: u32,
    pub genome_max_age_max: u32,
    pub genome_reproduction_rate_min: f32,
    pub genome_reproduction_rate_max: f32,
    pub genome_safe_reproduction_points_min: u32,
    pub genome_safe_reproduction_points_max: u32,
    // Aging/decay
    pub survival_cost_per_tick: u32,
    // Label visibility/thresholds
    pub show_collision_labels: bool,
    pub collision_label_force_min: f32, // display units (force / PPM)
    pub show_break_labels: bool,
    pub break_label_impulse_min: f32,   // raw impulse units
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuningUpdate {
    pub rel_vel_min: Option<f32>,
    pub rel_vel_max: Option<f32>,
    pub break_force_threshold: Option<f32>,
    pub energy_transfer_enabled: Option<bool>,
    pub energy_share_diff_threshold: Option<u32>,
    pub energy_share_friendly_rate: Option<f32>,
    pub energy_share_parent_not_friendly_child_friendly_rate: Option<f32>,
    pub energy_share_parent_friendly_child_not_friendly_rate: Option<f32>,
    pub energy_share_hostile_rand_min: Option<f32>,
    pub energy_share_hostile_rand_max: Option<f32>,
    pub bite_enabled: Option<bool>,
    pub bite_size_scale: Option<f32>,
    pub genome_bite_size_min: Option<u32>,
    pub genome_bite_size_max: Option<u32>,
    pub genome_energy_share_min: Option<f32>,
    pub genome_energy_share_max: Option<f32>,
    pub genome_friendly_distance_min: Option<f32>,
    pub genome_friendly_distance_max: Option<f32>,
    pub genome_friendly_scent_range: Option<f32>,
    pub genome_max_age_min: Option<u32>,
    pub genome_max_age_max: Option<u32>,
    pub genome_reproduction_rate_min: Option<f32>,
    pub genome_reproduction_rate_max: Option<f32>,
    pub genome_safe_reproduction_points_min: Option<u32>,
    pub genome_safe_reproduction_points_max: Option<u32>,
    pub survival_cost_per_tick: Option<u32>,
    pub show_collision_labels: Option<bool>,
    pub collision_label_force_min: Option<f32>,
    pub show_break_labels: Option<bool>,
    pub break_label_impulse_min: Option<f32>,
}

impl TuningUpdate {
    pub fn apply_to(self, t: &mut PhysicsTuning) {
        if let Some(v) = self.rel_vel_min { t.rel_vel_min = v; }
        if let Some(v) = self.rel_vel_max { t.rel_vel_max = v; }
        if let Some(v) = self.break_force_threshold { t.break_force_threshold = v; }
        if let Some(v) = self.energy_transfer_enabled { t.energy_transfer_enabled = v; }
        if let Some(v) = self.energy_share_diff_threshold { t.energy_share_diff_threshold = v; }
        if let Some(v) = self.energy_share_friendly_rate { t.energy_share_friendly_rate = v; }
        if let Some(v) = self.energy_share_parent_not_friendly_child_friendly_rate { t.energy_share_parent_not_friendly_child_friendly_rate = v; }
        if let Some(v) = self.energy_share_parent_friendly_child_not_friendly_rate { t.energy_share_parent_friendly_child_not_friendly_rate = v; }
        if let Some(v) = self.energy_share_hostile_rand_min { t.energy_share_hostile_rand_min = v; }
        if let Some(v) = self.energy_share_hostile_rand_max { t.energy_share_hostile_rand_max = v; }
        if let Some(v) = self.bite_enabled { t.bite_enabled = v; }
        if let Some(v) = self.bite_size_scale { t.bite_size_scale = v; }
        if let Some(v) = self.genome_bite_size_min { t.genome_bite_size_min = v; }
        if let Some(v) = self.genome_bite_size_max { t.genome_bite_size_max = v; }
        if let Some(v) = self.genome_energy_share_min { t.genome_energy_share_min = v; }
        if let Some(v) = self.genome_energy_share_max { t.genome_energy_share_max = v; }
        if let Some(v) = self.genome_friendly_distance_min { t.genome_friendly_distance_min = v; }
        if let Some(v) = self.genome_friendly_distance_max { t.genome_friendly_distance_max = v; }
        if let Some(v) = self.genome_friendly_scent_range { t.genome_friendly_scent_range = v; }
        if let Some(v) = self.genome_max_age_min { t.genome_max_age_min = v; }
        if let Some(v) = self.genome_max_age_max { t.genome_max_age_max = v; }
        if let Some(v) = self.genome_reproduction_rate_min { t.genome_reproduction_rate_min = v; }
        if let Some(v) = self.genome_reproduction_rate_max { t.genome_reproduction_rate_max = v; }
        if let Some(v) = self.genome_safe_reproduction_points_min { t.genome_safe_reproduction_points_min = v; }
        if let Some(v) = self.genome_safe_reproduction_points_max { t.genome_safe_reproduction_points_max = v; }
        if let Some(v) = self.survival_cost_per_tick { t.survival_cost_per_tick = v; }
        if let Some(v) = self.show_collision_labels { t.show_collision_labels = v; }
        if let Some(v) = self.collision_label_force_min { t.collision_label_force_min = v; }
        if let Some(v) = self.show_break_labels { t.show_break_labels = v; }
        if let Some(v) = self.break_label_impulse_min { t.break_label_impulse_min = v; }
    }
}

#[derive(Clone)]
struct AppState {
    tx: mpsc::Sender<TuningUpdate>,
    mirror: Arc<Mutex<PhysicsTuning>>, // for GET /tuning
}

async fn get_tuning(State(state): State<AppState>) -> Json<PhysicsTuning> {
    let guard = state.mirror.lock().unwrap();
    Json(guard.clone())
}

async fn patch_tuning(
    State(state): State<AppState>,
    Json(update): Json<TuningUpdate>,
) -> Json<PhysicsTuning> {
    // Send to Bevy for authoritative apply
    let _ = state.tx.send(update.clone());
    // Optimistically update mirror so GET reflects the change immediately
    {
        let mut guard = state.mirror.lock().unwrap();
        update.clone().apply_to(&mut guard);
        return Json(guard.clone());
    }
}

pub fn spawn_axum_server(
    addr: SocketAddr,
    tx: mpsc::Sender<TuningUpdate>,
    mirror: Arc<Mutex<PhysicsTuning>>,
) {
    std::thread::spawn(move || {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let state = AppState { tx, mirror };
            let app = Router::new()
                .route("/tuning", get(get_tuning).patch(patch_tuning))
                .with_state(state);

            let listener = tokio::net::TcpListener::bind(addr).await.expect("bind http");
            eprintln!("[diag] tuning server on http://{}", addr);
            axum::serve(listener, app).await.expect("serve http");
        });
    });
}

// Not a Resource; keep it plain to avoid Sync bound. We'll store it in a global once via insert_non_send_resource if needed.
pub struct TuningRx(pub mpsc::Receiver<TuningUpdate>);

#[derive(Resource, Clone)]
pub struct TuningMirror(pub Arc<Mutex<PhysicsTuning>>);

pub fn apply_tuning_updates_system(rx: bevy::prelude::NonSend<TuningRx>, mut tuning: ResMut<PhysicsTuning>, mirror: Res<TuningMirror>) {
    while let Ok(u) = rx.0.try_recv() {
        let upd = u.clone();
        upd.clone().apply_to(&mut tuning);
        if let Ok(mut g) = mirror.0.lock() { upd.apply_to(&mut g); }
    }
}

