use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder;

use bevy::prelude::{Resource, Res, ResMut};

// Hierarchical API structs for request/response JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTuning {
    pub stickiness: ApiStickiness,
    pub energy_share: ApiEnergyShare,
    pub bite: ApiBite,
    pub max_age: ApiMaxAge,
    pub reproduction: ApiReproduction,
    pub labels: ApiLabels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStickiness {
    pub stick_range: ApiStickRange,
    pub break_threshold: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStickRange { pub rel_vel_min: f32, pub rel_vel_max: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnergyShare {
    pub energy_transfer_enabled: bool,
    pub energy_share_diff_threshold: u32,
    pub genome_friendly_scent_range: f32,
    pub genome_friendly_distance_range: ApiGenomeFriendlyDistanceRange,
    pub energy_share_friendly_rate: f32,
    pub energy_share_parent_not_friendly_child_friendly_rate: f32,
    pub energy_share_parent_friendly_child_not_friendly_rate: f32,
    pub energy_share_hostile_rand_range: ApiEnergyShareHostileRandRange,
    pub genome_energy_share_range: ApiGenomeEnergyShareRange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGenomeFriendlyDistanceRange { pub genome_friendly_distance_min: f32, pub genome_friendly_distance_max: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnergyShareHostileRandRange { pub energy_share_hostile_rand_min: f32, pub energy_share_hostile_rand_max: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGenomeEnergyShareRange { pub genome_energy_share_min: f32, pub genome_energy_share_max: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBite {
    pub bite_enabled: bool,
    pub bite_size_scale: f32,
    pub genome_bite_size_range: ApiGenomeBiteSizeRange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGenomeBiteSizeRange { pub genome_bite_size_min: u32, pub genome_bite_size_max: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMaxAge {
    pub genome_max_age_range: ApiGenomeMaxAgeRange,
    pub survival_cost_per_tick: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGenomeMaxAgeRange { pub genome_max_age_min: u32, pub genome_max_age_max: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiReproduction {
    pub genome_reproduction_rate_range: ApiGenomeReproductionRateRange,
    pub genome_safe_reproduction_points_range: ApiGenomeSafeReproductionPointsRange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGenomeReproductionRateRange { pub genome_reproduction_rate_min: f32, pub genome_reproduction_rate_max: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGenomeSafeReproductionPointsRange { pub genome_safe_reproduction_points_min: u32, pub genome_safe_reproduction_points_max: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLabels {
    pub collision: ApiCollisionLabels,
    #[serde(rename = "break")] pub break_labels: ApiBreakLabels,
    pub age: ApiAgeLabels,
    pub energy: ApiEnergyLabels,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCollisionLabels { pub show_collision_labels: bool, pub collision_label_force_min: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBreakLabels { pub show_break_labels: bool, pub break_label_impulse_min: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAgeLabels { pub show_age_labels: bool, pub age_label_range: ApiAgeLabelRange }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAgeLabelRange { pub age_label_min: f32, pub age_label_max: f32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnergyLabels { pub show_energy_labels: bool, pub energy_label_range: ApiEnergyLabelRange }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnergyLabelRange { pub energy_label_min: f32, pub energy_label_max: f32 }

// Partial update types mirror ApiTuning with Options down to lowest level
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiTuningUpdate {
    pub stickiness: Option<ApiStickinessUpdate>,
    pub energy_share: Option<ApiEnergyShareUpdate>,
    pub bite: Option<ApiBiteUpdate>,
    pub max_age: Option<ApiMaxAgeUpdate>,
    pub reproduction: Option<ApiReproductionUpdate>,
    pub labels: Option<ApiLabelsUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiStickinessUpdate { pub stick_range: Option<ApiStickRangeUpdate>, pub break_threshold: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiStickRangeUpdate { pub rel_vel_min: Option<f32>, pub rel_vel_max: Option<f32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiEnergyShareUpdate {
    pub energy_transfer_enabled: Option<bool>,
    pub energy_share_diff_threshold: Option<u32>,
    pub genome_friendly_scent_range: Option<f32>,
    pub genome_friendly_distance_range: Option<ApiGenomeFriendlyDistanceRangeUpdate>,
    pub energy_share_friendly_rate: Option<f32>,
    pub energy_share_parent_not_friendly_child_friendly_rate: Option<f32>,
    pub energy_share_parent_friendly_child_not_friendly_rate: Option<f32>,
    pub energy_share_hostile_rand_range: Option<ApiEnergyShareHostileRandRangeUpdate>,
    pub genome_energy_share_range: Option<ApiGenomeEnergyShareRangeUpdate>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiGenomeFriendlyDistanceRangeUpdate { pub genome_friendly_distance_min: Option<f32>, pub genome_friendly_distance_max: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiEnergyShareHostileRandRangeUpdate { pub energy_share_hostile_rand_min: Option<f32>, pub energy_share_hostile_rand_max: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiGenomeEnergyShareRangeUpdate { pub genome_energy_share_min: Option<f32>, pub genome_energy_share_max: Option<f32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiBiteUpdate { pub bite_enabled: Option<bool>, pub bite_size_scale: Option<f32>, pub genome_bite_size_range: Option<ApiGenomeBiteSizeRangeUpdate> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiGenomeBiteSizeRangeUpdate { pub genome_bite_size_min: Option<u32>, pub genome_bite_size_max: Option<u32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiMaxAgeUpdate { pub genome_max_age_range: Option<ApiGenomeMaxAgeRangeUpdate>, pub survival_cost_per_tick: Option<u32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiGenomeMaxAgeRangeUpdate { pub genome_max_age_min: Option<u32>, pub genome_max_age_max: Option<u32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiReproductionUpdate {
    pub genome_reproduction_rate_range: Option<ApiGenomeReproductionRateRangeUpdate>,
    pub genome_safe_reproduction_points_range: Option<ApiGenomeSafeReproductionPointsRangeUpdate>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiGenomeReproductionRateRangeUpdate { pub genome_reproduction_rate_min: Option<f32>, pub genome_reproduction_rate_max: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiGenomeSafeReproductionPointsRangeUpdate { pub genome_safe_reproduction_points_min: Option<u32>, pub genome_safe_reproduction_points_max: Option<u32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiLabelsUpdate {
    pub collision: Option<ApiCollisionLabelsUpdate>,
    #[serde(rename = "break")] pub break_labels: Option<ApiBreakLabelsUpdate>,
    pub age: Option<ApiAgeLabelsUpdate>,
    pub energy: Option<ApiEnergyLabelsUpdate>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiCollisionLabelsUpdate { pub show_collision_labels: Option<bool>, pub collision_label_force_min: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiBreakLabelsUpdate { pub show_break_labels: Option<bool>, pub break_label_impulse_min: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiAgeLabelsUpdate { pub show_age_labels: Option<bool>, pub age_label_range: Option<ApiAgeLabelRangeUpdate> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiAgeLabelRangeUpdate { pub age_label_min: Option<f32>, pub age_label_max: Option<f32> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiEnergyLabelsUpdate { pub show_energy_labels: Option<bool>, pub energy_label_range: Option<ApiEnergyLabelRangeUpdate> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiEnergyLabelRangeUpdate { pub energy_label_min: Option<f32>, pub energy_label_max: Option<f32> }

impl ApiTuningUpdate {
    pub fn apply_to(self, t: &mut PhysicsTuning) {
        if let Some(st) = self.stickiness {
            if let Some(sr) = st.stick_range {
                if let Some(v) = sr.rel_vel_min { t.rel_vel_min = v; }
                if let Some(v) = sr.rel_vel_max { t.rel_vel_max = v; }
            }
            if let Some(v) = st.break_threshold { t.break_force_threshold = v; }
        }
        if let Some(es) = self.energy_share {
            if let Some(v) = es.energy_transfer_enabled { t.energy_transfer_enabled = v; }
            if let Some(v) = es.energy_share_diff_threshold { t.energy_share_diff_threshold = v; }
            if let Some(v) = es.genome_friendly_scent_range { t.genome_friendly_scent_range = v; }
            if let Some(gfdr) = es.genome_friendly_distance_range {
                if let Some(v) = gfdr.genome_friendly_distance_min { t.genome_friendly_distance_min = v; }
                if let Some(v) = gfdr.genome_friendly_distance_max { t.genome_friendly_distance_max = v; }
            }
            if let Some(v) = es.energy_share_friendly_rate { t.energy_share_friendly_rate = v; }
            if let Some(v) = es.energy_share_parent_not_friendly_child_friendly_rate { t.energy_share_parent_not_friendly_child_friendly_rate = v; }
            if let Some(v) = es.energy_share_parent_friendly_child_not_friendly_rate { t.energy_share_parent_friendly_child_not_friendly_rate = v; }
            if let Some(hrr) = es.energy_share_hostile_rand_range {
                if let Some(v) = hrr.energy_share_hostile_rand_min { t.energy_share_hostile_rand_min = v; }
                if let Some(v) = hrr.energy_share_hostile_rand_max { t.energy_share_hostile_rand_max = v; }
            }
            if let Some(ger) = es.genome_energy_share_range {
                if let Some(v) = ger.genome_energy_share_min { t.genome_energy_share_min = v; }
                if let Some(v) = ger.genome_energy_share_max { t.genome_energy_share_max = v; }
            }
        }
        if let Some(b) = self.bite {
            if let Some(v) = b.bite_enabled { t.bite_enabled = v; }
            if let Some(v) = b.bite_size_scale { t.bite_size_scale = v; }
            if let Some(gbr) = b.genome_bite_size_range {
                if let Some(v) = gbr.genome_bite_size_min { t.genome_bite_size_min = v; }
                if let Some(v) = gbr.genome_bite_size_max { t.genome_bite_size_max = v; }
            }
        }
        if let Some(ma) = self.max_age {
            if let Some(gmr) = ma.genome_max_age_range {
                if let Some(v) = gmr.genome_max_age_min { t.genome_max_age_min = v; }
                if let Some(v) = gmr.genome_max_age_max { t.genome_max_age_max = v; }
            }
            if let Some(v) = ma.survival_cost_per_tick { t.survival_cost_per_tick = v; }
        }
        if let Some(r) = self.reproduction {
            if let Some(rr) = r.genome_reproduction_rate_range {
                if let Some(v) = rr.genome_reproduction_rate_min { t.genome_reproduction_rate_min = v; }
                if let Some(v) = rr.genome_reproduction_rate_max { t.genome_reproduction_rate_max = v; }
            }
            if let Some(sr) = r.genome_safe_reproduction_points_range {
                if let Some(v) = sr.genome_safe_reproduction_points_min { t.genome_safe_reproduction_points_min = v; }
                if let Some(v) = sr.genome_safe_reproduction_points_max { t.genome_safe_reproduction_points_max = v; }
            }
        }
        if let Some(l) = self.labels {
            if let Some(c) = l.collision {
                if let Some(v) = c.show_collision_labels { t.show_collision_labels = v; }
                if let Some(v) = c.collision_label_force_min { t.collision_label_force_min = v; }
            }
            if let Some(b) = l.break_labels {
                if let Some(v) = b.show_break_labels { t.show_break_labels = v; }
                if let Some(v) = b.break_label_impulse_min { t.break_label_impulse_min = v; }
            }
            if let Some(a) = l.age {
                if let Some(v) = a.show_age_labels { t.show_age_labels = v; }
                if let Some(ar) = a.age_label_range {
                    if let Some(v) = ar.age_label_min { t.age_label_min = v; }
                    if let Some(v) = ar.age_label_max { t.age_label_max = v; }
                }
            }
            if let Some(e) = l.energy {
                if let Some(v) = e.show_energy_labels { t.show_energy_labels = v; }
                if let Some(er) = e.energy_label_range {
                    if let Some(v) = er.energy_label_min { t.energy_label_min = v; }
                    if let Some(v) = er.energy_label_max { t.energy_label_max = v; }
                }
            }
        }
    }
}

impl From<&PhysicsTuning> for ApiTuning {
    fn from(t: &PhysicsTuning) -> Self {
        ApiTuning {
            stickiness: ApiStickiness { stick_range: ApiStickRange { rel_vel_min: t.rel_vel_min, rel_vel_max: t.rel_vel_max }, break_threshold: t.break_force_threshold },
            energy_share: ApiEnergyShare {
                energy_transfer_enabled: t.energy_transfer_enabled,
                energy_share_diff_threshold: t.energy_share_diff_threshold,
                genome_friendly_scent_range: t.genome_friendly_scent_range,
                genome_friendly_distance_range: ApiGenomeFriendlyDistanceRange { genome_friendly_distance_min: t.genome_friendly_distance_min, genome_friendly_distance_max: t.genome_friendly_distance_max },
                energy_share_friendly_rate: t.energy_share_friendly_rate,
                energy_share_parent_not_friendly_child_friendly_rate: t.energy_share_parent_not_friendly_child_friendly_rate,
                energy_share_parent_friendly_child_not_friendly_rate: t.energy_share_parent_friendly_child_not_friendly_rate,
                energy_share_hostile_rand_range: ApiEnergyShareHostileRandRange { energy_share_hostile_rand_min: t.energy_share_hostile_rand_min, energy_share_hostile_rand_max: t.energy_share_hostile_rand_max },
                genome_energy_share_range: ApiGenomeEnergyShareRange { genome_energy_share_min: t.genome_energy_share_min, genome_energy_share_max: t.genome_energy_share_max },
            },
            bite: ApiBite { bite_enabled: t.bite_enabled, bite_size_scale: t.bite_size_scale, genome_bite_size_range: ApiGenomeBiteSizeRange { genome_bite_size_min: t.genome_bite_size_min, genome_bite_size_max: t.genome_bite_size_max } },
            max_age: ApiMaxAge { genome_max_age_range: ApiGenomeMaxAgeRange { genome_max_age_min: t.genome_max_age_min, genome_max_age_max: t.genome_max_age_max }, survival_cost_per_tick: t.survival_cost_per_tick },
            reproduction: ApiReproduction {
                genome_reproduction_rate_range: ApiGenomeReproductionRateRange { genome_reproduction_rate_min: t.genome_reproduction_rate_min, genome_reproduction_rate_max: t.genome_reproduction_rate_max },
                genome_safe_reproduction_points_range: ApiGenomeSafeReproductionPointsRange { genome_safe_reproduction_points_min: t.genome_safe_reproduction_points_min, genome_safe_reproduction_points_max: t.genome_safe_reproduction_points_max },
            },
            labels: ApiLabels {
                collision: ApiCollisionLabels { show_collision_labels: t.show_collision_labels, collision_label_force_min: t.collision_label_force_min },
                break_labels: ApiBreakLabels { show_break_labels: t.show_break_labels, break_label_impulse_min: t.break_label_impulse_min },
                age: ApiAgeLabels { show_age_labels: t.show_age_labels, age_label_range: ApiAgeLabelRange { age_label_min: t.age_label_min, age_label_max: t.age_label_max } },
                energy: ApiEnergyLabels { show_energy_labels: t.show_energy_labels, energy_label_range: ApiEnergyLabelRange { energy_label_min: t.energy_label_min, energy_label_max: t.energy_label_max } },
            },
        }
    }
}

impl From<ApiTuning> for PhysicsTuning {
    fn from(api: ApiTuning) -> Self {
        PhysicsTuning {
            rel_vel_min: api.stickiness.stick_range.rel_vel_min,
            rel_vel_max: api.stickiness.stick_range.rel_vel_max,
            break_force_threshold: api.stickiness.break_threshold,
            energy_transfer_enabled: api.energy_share.energy_transfer_enabled,
            energy_share_diff_threshold: api.energy_share.energy_share_diff_threshold,
            energy_share_friendly_rate: api.energy_share.energy_share_friendly_rate,
            energy_share_parent_not_friendly_child_friendly_rate: api.energy_share.energy_share_parent_not_friendly_child_friendly_rate,
            energy_share_parent_friendly_child_not_friendly_rate: api.energy_share.energy_share_parent_friendly_child_not_friendly_rate,
            energy_share_hostile_rand_min: api.energy_share.energy_share_hostile_rand_range.energy_share_hostile_rand_min,
            energy_share_hostile_rand_max: api.energy_share.energy_share_hostile_rand_range.energy_share_hostile_rand_max,
            bite_enabled: api.bite.bite_enabled,
            bite_size_scale: api.bite.bite_size_scale,
            genome_bite_size_min: api.bite.genome_bite_size_range.genome_bite_size_min,
            genome_bite_size_max: api.bite.genome_bite_size_range.genome_bite_size_max,
            genome_energy_share_min: api.energy_share.genome_energy_share_range.genome_energy_share_min,
            genome_energy_share_max: api.energy_share.genome_energy_share_range.genome_energy_share_max,
            genome_friendly_distance_min: api.energy_share.genome_friendly_distance_range.genome_friendly_distance_min,
            genome_friendly_distance_max: api.energy_share.genome_friendly_distance_range.genome_friendly_distance_max,
            genome_friendly_scent_range: api.energy_share.genome_friendly_scent_range,
            genome_max_age_min: api.max_age.genome_max_age_range.genome_max_age_min,
            genome_max_age_max: api.max_age.genome_max_age_range.genome_max_age_max,
            genome_reproduction_rate_min: api.reproduction.genome_reproduction_rate_range.genome_reproduction_rate_min,
            genome_reproduction_rate_max: api.reproduction.genome_reproduction_rate_range.genome_reproduction_rate_max,
            genome_safe_reproduction_points_min: api.reproduction.genome_safe_reproduction_points_range.genome_safe_reproduction_points_min,
            genome_safe_reproduction_points_max: api.reproduction.genome_safe_reproduction_points_range.genome_safe_reproduction_points_max,
            survival_cost_per_tick: api.max_age.survival_cost_per_tick,
            show_collision_labels: api.labels.collision.show_collision_labels,
            collision_label_force_min: api.labels.collision.collision_label_force_min,
            show_break_labels: api.labels.break_labels.show_break_labels,
            break_label_impulse_min: api.labels.break_labels.break_label_impulse_min,
            show_age_labels: api.labels.age.show_age_labels,
            age_label_min: api.labels.age.age_label_range.age_label_min,
            age_label_max: api.labels.age.age_label_range.age_label_max,
            show_energy_labels: api.labels.energy.show_energy_labels,
            energy_label_min: api.labels.energy.energy_label_range.energy_label_min,
            energy_label_max: api.labels.energy.energy_label_range.energy_label_max,
        }
    }
}

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
    // New: Age/Energy labels
    pub show_age_labels: bool,
    pub age_label_min: f32,
    pub age_label_max: f32,
    pub show_energy_labels: bool,
    pub energy_label_min: f32,
    pub energy_label_max: f32,
}


#[derive(Clone)]
struct AppState {
    tx: mpsc::Sender<PhysicsTuning>,
    mirror: Arc<Mutex<PhysicsTuning>>, // for GET /tuning
}

async fn get_tuning(State(state): State<AppState>) -> Json<ApiTuning> {
    let guard = state.mirror.lock().unwrap();
    Json(ApiTuning::from(&*guard))
}

async fn patch_tuning(
    State(state): State<AppState>,
    Json(api_update): Json<ApiTuningUpdate>,
) -> Json<ApiTuning> {
    // Apply partial update into current tuning
    let new_tuning = {
        let mut guard = state.mirror.lock().unwrap();
        api_update.clone().apply_to(&mut guard);
        guard.clone()
    };
    // Send to Bevy for authoritative apply
    let _ = state.tx.send(new_tuning.clone());
    // Return current mirror as hierarchical response
    {
        let guard = state.mirror.lock().unwrap();
        Json(ApiTuning::from(&*guard))
    }
}

pub fn spawn_axum_server(
    addr: SocketAddr,
    tx: mpsc::Sender<PhysicsTuning>,
    mirror: Arc<Mutex<PhysicsTuning>>,
) {
    std::thread::spawn(move || {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let app = build_router(tx, mirror);

            let listener = tokio::net::TcpListener::bind(addr).await.expect("bind http");
            eprintln!("[diag] tuning server on http://{}", addr);
            axum::serve(listener, app).await.expect("serve http");
        });
    });
}

fn build_router(tx: mpsc::Sender<PhysicsTuning>, mirror: Arc<Mutex<PhysicsTuning>>) -> Router {
    let state = AppState { tx, mirror };
    Router::new()
        .route("/tuning", get(get_tuning).patch(patch_tuning))
        .with_state(state)
}

pub fn build_router_for_test(tx: mpsc::Sender<PhysicsTuning>, mirror: Arc<Mutex<PhysicsTuning>>) -> Router {
    build_router(tx, mirror)
}

// Not a Resource; keep it plain to avoid Sync bound. We'll store it in a global once via insert_non_send_resource if needed.
pub struct TuningRx(pub mpsc::Receiver<PhysicsTuning>);


#[derive(Resource, Clone)]
pub struct TuningMirror(pub Arc<Mutex<PhysicsTuning>>);

pub fn apply_tuning_updates_system(rx: bevy::prelude::NonSend<TuningRx>, mut tuning: ResMut<PhysicsTuning>, mirror: Res<TuningMirror>) {
    while let Ok(new_tuning) = rx.0.try_recv() {
        // Update Bevy resource
        *tuning = new_tuning.clone();
        // Update mirror
        if let Ok(mut g) = mirror.0.lock() { *g = new_tuning.clone(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_physics() -> PhysicsTuning {
        PhysicsTuning {
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
        }
    }

    #[test]
    fn conversion_roundtrip() {
        let internal = sample_physics();
        let api: ApiTuning = (&internal).into();
        let back: PhysicsTuning = api.into();
        assert_eq!(internal.rel_vel_min, back.rel_vel_min);
        assert_eq!(internal.rel_vel_max, back.rel_vel_max);
        assert_eq!(internal.break_force_threshold, back.break_force_threshold);
        assert_eq!(internal.energy_transfer_enabled, back.energy_transfer_enabled);
        assert_eq!(internal.energy_share_diff_threshold, back.energy_share_diff_threshold);
        assert_eq!(internal.energy_share_friendly_rate, back.energy_share_friendly_rate);
        assert_eq!(internal.energy_share_parent_not_friendly_child_friendly_rate, back.energy_share_parent_not_friendly_child_friendly_rate);
        assert_eq!(internal.energy_share_parent_friendly_child_not_friendly_rate, back.energy_share_parent_friendly_child_not_friendly_rate);
        assert_eq!(internal.energy_share_hostile_rand_min, back.energy_share_hostile_rand_min);
        assert_eq!(internal.energy_share_hostile_rand_max, back.energy_share_hostile_rand_max);
        assert_eq!(internal.bite_enabled, back.bite_enabled);
        assert_eq!(internal.bite_size_scale, back.bite_size_scale);
        assert_eq!(internal.genome_bite_size_min, back.genome_bite_size_min);
        assert_eq!(internal.genome_bite_size_max, back.genome_bite_size_max);
    }

    #[test]
    fn partial_update_apply() {
        let mut internal = PhysicsTuning { rel_vel_min: 0.1, rel_vel_max: 10.0, ..Default::default() };
        let upd = ApiTuningUpdate {
            stickiness: Some(ApiStickinessUpdate {
                stick_range: Some(ApiStickRangeUpdate { rel_vel_min: Some(1.23), rel_vel_max: None }),
                break_threshold: Some(42.0),
            }),
            labels: Some(ApiLabelsUpdate {
                collision: Some(ApiCollisionLabelsUpdate { show_collision_labels: Some(true), collision_label_force_min: Some(3.3) }),
                break_labels: None,
                age: Some(ApiAgeLabelsUpdate { show_age_labels: Some(true), age_label_range: Some(ApiAgeLabelRangeUpdate { age_label_min: Some(5.0), age_label_max: None }) }),
                energy: Some(ApiEnergyLabelsUpdate { show_energy_labels: Some(true), energy_label_range: Some(ApiEnergyLabelRangeUpdate { energy_label_min: None, energy_label_max: Some(900.0) }) }),
            }),
            ..Default::default()
        };
        upd.apply_to(&mut internal);
        assert_eq!(internal.rel_vel_min, 1.23);
        assert_eq!(internal.rel_vel_max, 10.0); // unchanged

    // Integration-style test lives in tests/ for nameability

        assert_eq!(internal.break_force_threshold, 42.0);
        assert!(internal.show_collision_labels);
        assert_eq!(internal.collision_label_force_min, 3.3);
        assert!(internal.show_age_labels);
        assert_eq!(internal.age_label_min, 5.0);
        assert_eq!(internal.energy_label_max, 900.0);
        assert!(internal.show_energy_labels);
    }
}


