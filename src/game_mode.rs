use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::AlsInputMap;
use als_core::params::AlsMovementParams;
use als_core::player_start::{
    AlsPlayerStart, AlsPlayerStartCandidate, AlsPlayerStartReady, AlsPlayerStartSelection,
    AlsPlayerStartSettings, AlsPlayerStartOccupancy, PLAYER_START_TELEPORT_SKIN,
    choose_player_start, teleport_xy_adjustments,
};
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
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
            .init_resource::<AlsPlayerStartSettings>()
            .add_systems(Update, admit_pending_players.after(als_core::player_start::AlsPlayerStartPrepared));
    }
}

fn spawn_capsules_overlap(a: Vec3, b: Vec3, half_height: f32, radius: f32) -> bool {
    let horizontal = Vec2::new(a.x - b.x, a.z - b.z).length_squared();
    let vertical = ((a.y - b.y).abs() - 2.0 * (half_height - radius).max(0.0)).max(0.0);
    horizontal + vertical * vertical < (2.0 * radius).powi(2)
}

fn fit_reserved_player_start(
    candidate: &AlsPlayerStartCandidate,
    reserved: &[Vec3],
    settings: &AlsPlayerStartSettings,
    spatial: &SpatialQuery,
) -> Option<AlsPlayerStartCandidate> {
    let center = candidate.fit.transform.translation;
    if reserved.iter().all(|position| !spawn_capsules_overlap(
        center, *position, settings.capsule_half_height, settings.capsule_radius,
    )) {
        return Some(candidate.clone());
    }
    let shape = als_core::character::als_character_capsule(
        settings.capsule_half_height, settings.capsule_radius,
    );
    let filter = SpatialQueryFilter::default()
        .with_mask(als_core::character::AlsCollisionLayer::Default);
    for blocker in reserved.iter().filter(|position| spawn_capsules_overlap(
        center, **position, settings.capsule_half_height, settings.capsule_radius,
    )) {
        let delta = Vec2::new(center.x - blocker.x, center.z - blocker.z);
        let shift = if delta.length_squared() > 1e-6 {
            delta.normalize() * (2.0 * settings.capsule_radius - delta.length() + PLAYER_START_TELEPORT_SKIN)
        } else {
            Vec2::X * (2.0 * settings.capsule_radius + PLAYER_START_TELEPORT_SKIN)
        };
        for xy in teleport_xy_adjustments(shift) {
            let mut adjusted = candidate.clone();
            let offset = Vec3::new(xy.x, 0.0, xy.y);
            adjusted.fit.transform.translation += offset;
            if reserved.iter().any(|position| spawn_capsules_overlap(
                adjusted.fit.transform.translation, *position,
                settings.capsule_half_height, settings.capsule_radius,
            )) || !spatial.shape_intersections(
                &shape, adjusted.fit.transform.translation, adjusted.fit.transform.rotation, &filter,
            ).is_empty() {
                continue;
            }
            adjusted.fit.teleport_adjustment += offset;
            adjusted.fit.occupancy = AlsPlayerStartOccupancy::OccupiedAdjusted;
            return Some(adjusted);
        }
    }
    None
}

#[allow(clippy::type_complexity)]
fn admit_pending_players(
    mut commands: Commands,
    barrier: Option<Res<AlsScenePhysicsReady>>,
    settings: Res<AlsPlayerStartSettings>,
    spatial: SpatialQuery,
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
    let mut reserved = Vec::new();
    for (request, admission) in &pending {
        let available: Vec<_> = candidates.iter()
            .filter_map(|candidate| fit_reserved_player_start(candidate, &reserved, &settings, &spatial))
            .collect();
        let tag = admission.incoming_start_tag.as_deref().filter(|tag| !tag.is_empty());
        let tagged_reserved = tag.is_some_and(|tag| {
            candidates.iter().any(|candidate| candidate.tag.as_deref() == Some(tag))
                && !available.iter().any(|candidate| candidate.tag.as_deref() == Some(tag))
        });
        let selection = (!tagged_reserved)
            .then(|| choose_player_start(&available, tag, random.next_ticket()))
            .flatten();
        let Some(selection) = selection else {
            commands.entity(request).remove::<AlsPlayerAdmissionPending>().insert(
                AlsPlayerAdmissionFailed { reason: "No PlayerStart can fit the default pawn".into() },
            );
            continue;
        };
        reserved.push(selection.fit.transform.translation);
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
    use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
    use bevy::asset::AssetPlugin;
    use bevy_animation_graph::core::animated_scene::AnimatedScene;

    #[test]
    fn same_frame_admissions_do_not_share_one_capsule_space() {
        assert!(spawn_capsules_overlap(Vec3::ZERO, Vec3::ZERO, 0.9, 0.3));
        assert!(!spawn_capsules_overlap(Vec3::ZERO, Vec3::X * 0.61, 0.9, 0.3));
        assert!(!spawn_capsules_overlap(Vec3::ZERO, Vec3::Y * 1.81, 0.9, 0.3));
        for (starts, incoming_tag, blocked) in [
            (vec![(Vec3::ZERO, None)], None, false),
            (vec![(Vec3::ZERO, Some("Portal")), (Vec3::X * 3.0, Some("Other"))], Some("Portal"), false),
            (vec![(Vec3::ZERO, None), (Vec3::X * 3.0, None)], None, false),
            (vec![(Vec3::ZERO, None)], None, true),
        ] {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                bevy::mesh::MeshPlugin,
                bevy::transform::TransformPlugin,
                PhysicsPlugins::new(FixedPostUpdate),
                AlsGameModePlugin,
            ))
                .init_asset::<StandardMaterial>()
                .init_asset::<AlsMovementParams>()
                .init_asset::<AlsControlSchemeConfig>()
                .init_asset::<AnimatedScene>()
                .init_asset::<als_camera::AlsCameraDefinition>();
            app.finish();
            app.cleanup();
            let mut starts_spawned = Vec::new();
            for (offset, tag) in starts {
                starts_spawned.push(app.world_mut().spawn((
                    AlsPlayerStart { tag: tag.map(str::to_owned), ..default() },
                    AlsPlayerStartReady {
                        transform: Transform::from_translation(offset + Vec3::Y * 0.919),
                        ground_y: 0.0,
                        adjustment: 0.0,
                        teleport_adjustment: Vec3::ZERO,
                        occupancy: AlsPlayerStartOccupancy::Unoccupied,
                    },
                )).id());
            }
            if blocked {
                for offset in teleport_xy_adjustments(Vec2::X * (0.6 + PLAYER_START_TELEPORT_SKIN)) {
                    app.world_mut().spawn((
                        RigidBody::Static,
                        Collider::cuboid(0.1, 1.8, 0.1),
                        Transform::from_xyz(offset.x, 0.919, offset.y),
                    ));
                }
                app.update();
                app.world_mut().run_schedule(FixedPostUpdate);
                app.update();
            }
            let admission = AlsPlayerAdmission {
                spawn_input: false,
                incoming_start_tag: incoming_tag.map(str::to_owned),
                ..AlsPlayerAdmission::new()
            };
            let first = app.world_mut().spawn((admission.clone(), AlsPlayerAdmissionPending)).id();
            let second = app.world_mut().spawn((admission, AlsPlayerAdmissionPending)).id();
            app.update();
            let first_spawn = app.world().get::<AlsPlayerAdmissionComplete>(first).unwrap();
            if blocked {
                assert!(app.world().get::<AlsPlayerAdmissionComplete>(second).is_none());
                assert!(app.world().get::<AlsPlayerAdmissionFailed>(second).is_some());
                continue;
            }
            let second_spawn = app.world().get::<AlsPlayerAdmissionComplete>(second).unwrap();
            let first_position = app.world().get::<Transform>(first_spawn.character).unwrap().translation;
            let second_position = app.world().get::<Transform>(second_spawn.character).unwrap().translation;
            assert!(first_position.distance(second_position) >= 0.6);
            if starts_spawned.len() == 1 || incoming_tag.is_some() {
                assert_eq!(first_spawn.selection.entity, starts_spawned[0]);
                assert_eq!(second_spawn.selection.entity, starts_spawned[0]);
                assert_eq!(second_spawn.selection.fit.occupancy, AlsPlayerStartOccupancy::OccupiedAdjusted);
            } else {
                assert_ne!(first_spawn.selection.entity, second_spawn.selection.entity);
            }
        }
    }
}
