use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::AlsInputMap;
use als_core::params::AlsMovementParams;
use als_core::player_start::{
    AlsPlayerStart, AlsPlayerStartCandidate, AlsPlayerStartReady, AlsPlayerStartSelection,
    choose_player_start,
};
use bevy::prelude::*;

use crate::character_assembly::{AlsCharacterBlueprint, spawn_als_character_with_materials};

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsScenePhysicsReady {
    pub ready: bool,
}

impl Default for AlsScenePhysicsReady {
    fn default() -> Self {
        Self { ready: false }
    }
}

#[derive(Component, Debug, Clone)]
pub struct AlsPlayerAdmission {
    pub input_map: AlsInputMap,
    pub spawn_input: bool,
    pub spawn_camera: bool,
    pub incoming_start_tag: Option<String>,
}

impl AlsPlayerAdmission {
    pub fn new() -> Self {
        Self {
            input_map: AlsInputMap::default(),
            spawn_input: true,
            spawn_camera: false,
            incoming_start_tag: None,
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsPlayerAdmissionPending;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct AlsPlayerAdmissionComplete {
    pub character: Entity,
    pub selection: AlsPlayerStartSelection,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct AlsPlayerAdmissionFailed {
    pub reason: String,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct AlsGameModeRandom {
    state: u64,
}

impl Default for AlsGameModeRandom {
    fn default() -> Self {
        Self {
            state: 0xA1_5A_1C_0D_3E_4F_5A_6B,
        }
    }
}

impl AlsGameModeRandom {
    fn next_ticket(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

pub fn request_als_player(commands: &mut Commands, admission: AlsPlayerAdmission) -> Entity {
    commands.spawn((admission, AlsPlayerAdmissionPending)).id()
}

pub struct AlsGameModePlugin;

impl Plugin for AlsGameModePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsGameModeRandom>()
            .add_systems(Update, admit_pending_players);
    }
}

#[allow(clippy::type_complexity)]
fn admit_pending_players(
    mut commands: Commands,
    barrier: Option<Res<AlsScenePhysicsReady>>,
    mut random: ResMut<AlsGameModeRandom>,
    all_starts: Query<Entity, With<AlsPlayerStart>>,
    starts: Query<(Entity, &AlsPlayerStart, &AlsPlayerStartReady)>,
    pending: Query<(Entity, &AlsPlayerAdmission), With<AlsPlayerAdmissionPending>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    if barrier.as_ref().is_some_and(|barrier| !barrier.ready) {
        return;
    }
    let candidates: Vec<AlsPlayerStartCandidate> = starts
        .iter()
        .map(|(entity, start, ready)| AlsPlayerStartCandidate {
            entity,
            tag: start.tag.clone(),
            fit: als_core::player_start::AlsPlayerSpawnFit {
                transform: ready.transform,
                ground_y: ready.ground_y,
                adjustment: ready.adjustment,
                teleport_adjustment: ready.teleport_adjustment,
                occupancy: ready.occupancy,
            },
        })
        .collect();
    if candidates.is_empty() {
        if barrier.as_ref().is_some_and(|barrier| barrier.ready) && !all_starts.is_empty() {
            for (request, _) in &pending {
                commands
                    .entity(request)
                    .remove::<AlsPlayerAdmissionPending>();
                commands.entity(request).insert(AlsPlayerAdmissionFailed {
                    reason: "No PlayerStart can fit the default pawn".into(),
                });
                info!("[GAMEMODE][DENY] request={request:?} reason=no_fit -> PASS");
            }
        }
        return;
    }
    for (request, admission) in &pending {
        let Some(selection) = choose_player_start(
            &candidates,
            admission.incoming_start_tag.as_deref(),
            random.next_ticket(),
        ) else {
            continue;
        };
        let character = spawn_als_character_with_materials(
            &mut commands,
            &assets,
            &mut materials,
            &mut params_assets,
            &mut config_assets,
            AlsCharacterBlueprint {
                transform: selection.fit.transform,
                input_map: admission.input_map.clone(),
                spawn_input: admission.spawn_input,
                spawn_camera: admission.spawn_camera,
            },
        );
        info!(
            "[GAMEMODE][ADMIT] request={request:?} start={:?} occupancy={:?} fallback={} center=({:.4},{:.4},{:.4}) yaw={:.4} -> PASS",
            selection.entity,
            selection.fit.occupancy,
            selection.used_occupied_fallback,
            selection.fit.transform.translation.x,
            selection.fit.transform.translation.y,
            selection.fit.transform.translation.z,
            selection.fit.transform.rotation.to_euler(EulerRot::YXZ).0,
        );
        commands
            .entity(request)
            .remove::<AlsPlayerAdmissionPending>();
        commands.entity(request).insert(AlsPlayerAdmissionComplete {
            character,
            selection,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use als_core::player_start::AlsPlayerStartOccupancy;
    use bevy::asset::AssetPlugin;
    use bevy_animation_graph::core::animated_scene::AnimatedScene;

    #[test]
    #[ignore = "A16: same-frame admissions reuse unreserved spawn space"]
    fn same_frame_admissions_do_not_share_one_capsule_space() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AlsGameModePlugin))
            .init_asset::<StandardMaterial>()
            .init_asset::<AlsMovementParams>()
            .init_asset::<AlsControlSchemeConfig>()
            .init_asset::<AnimatedScene>()
            .init_asset::<als_camera::AlsCameraDefinition>();
        app.finish();
        app.cleanup();
        let transform = Transform::from_xyz(0.0, 0.919, 0.0);
        app.world_mut().spawn((
            AlsPlayerStart::default(),
            AlsPlayerStartReady {
                transform,
                ground_y: 0.0,
                adjustment: 0.0,
                teleport_adjustment: Vec3::ZERO,
                occupancy: AlsPlayerStartOccupancy::Unoccupied,
            },
        ));
        let admission = AlsPlayerAdmission {
            spawn_input: false,
            ..AlsPlayerAdmission::new()
        };
        let first = app.world_mut().spawn((admission.clone(), AlsPlayerAdmissionPending)).id();
        let second = app.world_mut().spawn((admission, AlsPlayerAdmissionPending)).id();
        app.update();
        let first_spawn = app.world().get::<AlsPlayerAdmissionComplete>(first).unwrap();
        if let Some(second_spawn) = app.world().get::<AlsPlayerAdmissionComplete>(second) {
            let first_position = app.world().get::<Transform>(first_spawn.character).unwrap().translation;
            let second_position = app.world().get::<Transform>(second_spawn.character).unwrap().translation;
            assert!(first_position.distance(second_position) >= 0.6);
        } else {
            assert!(app.world().get::<AlsPlayerAdmissionFailed>(second).is_some());
        }
    }
}
