use crate::als::character::AlsCharacter;
use crate::als::foot_ik::{AlsFootLockState, AlsFootTargetSelection};
use crate::als::input_request::AlsFirstPersonView;
use crate::als::orientation::world_forward_to_yaw;
use crate::als::state::RotationMode;
use crate::control_rig_query::{RigQueryContext, RigQueryWorld};
use crate::extensions::source_instance::source_player_property;
use crate::extensions::{
    ShapeCollisionMode, control_rig_hierarchy::RigTransform,
    cooked_mesh_raycast::CookedMeshRayData, source_instance::set_source_frame_input,
};
use avian3d::{
    collider_tree::ColliderTrees,
    prelude::{Collider, CollisionLayers, Position, Rotation, SpatialQueryFilter},
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy::transform::systems as transform_systems;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
#[cfg(test)]
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use serde::Deserialize;
use std::sync::Arc;

use crate::actual_motion::AlsActualMotion;
use crate::animation_context_projection::AlsRootAnimationContext;
use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar, character_mesh_entity};
use crate::foot_ik_apply::FootIkApplySet;
use crate::head_layer::{AlsHeadLayerSet, inv_exp_approx, unwind_degrees};

pub(crate) const CR_ALS_RUNTIME_PROFILE: &str =
    include_str!("../../../assets/ALS/Character/CR_Als.runtime.json");
const SPINE_ENTER_HALF_LIFE: f32 = 0.1;
const SPINE_EXIT_HALF_LIFE: f32 = 0.7;
const SPINE_REFERENCE_VIEW_YAW_SPEED: f32 = 40.0;
const SPINE_MAX_WORLD_OFFSET: f32 = 30.0;

#[derive(Clone, Deserialize)]
pub(crate) struct CrAlsRuntimeProfileDocument {
    pub(crate) target: CrAlsTargetProfile,
}

#[derive(Clone, Deserialize)]
pub(crate) struct CrAlsTargetProfile {
    pub(crate) foot_left_thigh_axis_pelvis_space: [f32; 3],
    pub(crate) foot_right_thigh_axis_pelvis_space: [f32; 3],
}

#[derive(Resource, Clone)]
pub(crate) struct CrAlsRuntimeProfileTemplate(pub(crate) CrAlsRuntimeProfileDocument);

impl FromWorld for CrAlsRuntimeProfileTemplate {
    fn from_world(_world: &mut World) -> Self {
        Self(
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE)
                .expect("foot lock reference axes must parse"),
        )
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigInputSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsControlRigPropagationSet;

pub struct ControlRigApplyPlugin;

impl Plugin for ControlRigApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                AlsControlRigInputSet.after(AlsHeadLayerSet).before(
                    crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
                ),
                crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Readback
                    .after(FootIkApplySet::Telemetry),
                AlsControlRigPropagationSet.after(
                    crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Readback,
                ),
            ),
        )
        .add_systems(
            PostUpdate,
            (
                initialize_control_rig_input
                    .before(FootIkApplySet::UpdateInputs)
                    .before(AlsControlRigInputSet),
                (refresh_control_rig_input, publish_control_rig_inputs)
                    .chain()
                    .in_set(AlsControlRigInputSet),
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .in_set(AlsControlRigPropagationSet),
            ),
        );
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlsControlRigSettings {
    pub use_hand_ik_bones: bool,
    pub use_foot_ik_bones: bool,
}

impl Default for AlsControlRigSettings {
    fn default() -> Self {
        Self {
            use_hand_ik_bones: true,
            use_foot_ik_bones: true,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsControlRigExecution {
    pub initialization_count: u64,
    pub executed_steps: usize,
    pub frame: u64,
    pub elapsed_seconds: f64,
    pub foot_height_m: f32,
    pub leg_length_m: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsControlRigInput {
    pub use_hand_ik_bones: bool,
    pub use_foot_ik_bones: bool,
    pub foot_transforms_valid: bool,
    pub velocity_blend_forward_amount: f32,
    pub velocity_blend_backward_amount: f32,
    pub spine_yaw_angle_degrees: f32,
    pub pelvis_offset_amount: f32,
    pub foot_left_location_armature: Vec3,
    pub foot_left_rotation_armature: Quat,
    pub foot_right_location_armature: Vec3,
    pub foot_right_rotation_armature: Quat,
}

#[derive(Component)]
#[doc(hidden)]
pub struct AlsControlRigFootLocationsCm(pub(crate) [Vec3; 2]);

impl Default for AlsControlRigInput {
    fn default() -> Self {
        Self {
            use_hand_ik_bones: true,
            use_foot_ik_bones: true,
            foot_transforms_valid: false,
            velocity_blend_forward_amount: 0.0,
            velocity_blend_backward_amount: 0.0,
            spine_yaw_angle_degrees: 0.0,
            pelvis_offset_amount: 0.0,
            foot_left_location_armature: Vec3::ZERO,
            foot_left_rotation_armature: Quat::IDENTITY,
            foot_right_location_armature: Vec3::ZERO,
            foot_right_rotation_armature: Quat::IDENTITY,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsControlRigState {
    pub spine_rotation_allowed: bool,
    pub spine_amount: f32,
    pub spine_amount_scale: f32,
    pub spine_amount_bias: f32,
    pub last_yaw_angle: f32,
    pub last_yaw_angle_world_space: f32,
    pub movement_base_snapshot: Option<(Entity, Quat)>,
    pub yaw_angle: f32,
    pub final_yaw_angle: f32,
}

impl Default for AlsControlRigState {
    fn default() -> Self {
        Self {
            spine_rotation_allowed: false,
            spine_amount: 0.0,
            spine_amount_scale: 1.0,
            spine_amount_bias: 0.0,
            last_yaw_angle: 0.0,
            last_yaw_angle_world_space: 0.0,
            movement_base_snapshot: None,
            yaw_angle: 0.0,
            final_yaw_angle: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct SpineInput {
    allowed: bool,
    pending_update: bool,
    view_yaw_angle: f32,
    view_yaw_speed: f32,
    body_yaw_world: f32,
    movement_base_delta_yaw: f32,
    blend_amount: f32,
    delta_seconds: f32,
}

fn is_relevant(value: f32) -> bool {
    value > 1e-5
}

fn is_full(value: f32) -> bool {
    value >= 1.0 - 1e-5
}

fn damper_exact(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let alpha = 1.0 - inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * delta_seconds);
    current + (target - current) * alpha
}

fn lerp_angle(from: f32, to: f32, amount: f32) -> f32 {
    let delta = unwind_degrees(to - from);
    unwind_degrees(from + (if delta > 175.0 { delta - 360.0 } else { delta }) * amount)
}

fn update_spine_state(state: &mut AlsControlRigState, input: SpineInput) {
    if state.spine_rotation_allowed != input.allowed {
        state.spine_rotation_allowed = input.allowed;
        if state.spine_rotation_allowed {
            if is_full(state.spine_amount) {
                state.spine_amount_scale = 1.0;
                state.spine_amount_bias = 0.0;
            } else {
                state.spine_amount_scale = 1.0 / (1.0 - state.spine_amount);
                state.spine_amount_bias = -state.spine_amount * state.spine_amount_scale;
            }
        } else {
            state.spine_amount_scale = if is_relevant(state.spine_amount) {
                1.0 / state.spine_amount
            } else {
                1.0
            };
            state.spine_amount_bias = 0.0;
        }
        state.last_yaw_angle = state.yaw_angle;
        state.last_yaw_angle_world_space = input.body_yaw_world;
    }
    if state.spine_rotation_allowed {
        if input.pending_update || is_full(state.spine_amount) {
            state.spine_amount = 1.0;
            state.yaw_angle = input.view_yaw_angle;
        } else {
            state.spine_amount = damper_exact(
                state.spine_amount,
                1.0,
                input.delta_seconds,
                SPINE_ENTER_HALF_LIFE,
            );
            state.yaw_angle = lerp_angle(
                state.last_yaw_angle,
                input.view_yaw_angle,
                state.spine_amount * state.spine_amount_scale + state.spine_amount_bias,
            );
        }
    } else if input.pending_update || !is_relevant(state.spine_amount) {
        state.spine_amount = 0.0;
        state.yaw_angle = 0.0;
    } else {
        let multiplier = if input.view_yaw_speed > SPINE_REFERENCE_VIEW_YAW_SPEED {
            SPINE_REFERENCE_VIEW_YAW_SPEED / input.view_yaw_speed
        } else {
            1.0
        };
        state.spine_amount = damper_exact(
            state.spine_amount,
            0.0,
            input.delta_seconds,
            SPINE_EXIT_HALF_LIFE * multiplier,
        );
        state.last_yaw_angle_world_space =
            unwind_degrees(state.last_yaw_angle_world_space + input.movement_base_delta_yaw);
        let yaw_offset = unwind_degrees(state.last_yaw_angle_world_space - input.body_yaw_world)
            .clamp(-SPINE_MAX_WORLD_OFFSET, SPINE_MAX_WORLD_OFFSET);
        state.last_yaw_angle_world_space = unwind_degrees(yaw_offset + input.body_yaw_world);
        state.yaw_angle = lerp_angle(
            0.0,
            state.last_yaw_angle + yaw_offset,
            state.spine_amount * state.spine_amount_scale + state.spine_amount_bias,
        );
    }
    state.final_yaw_angle = lerp_angle(0.0, state.yaw_angle, input.blend_amount);
}

fn output_f32(context: Option<&AlsRootAnimationContext>, name: &str) -> f32 {
    context
        .and_then(|context| context.f32(name))
        .unwrap_or_default()
}

fn initialize_control_rig_input(
    mut commands: Commands,
    characters: Query<
        (
            Entity,
            Option<&AlsControlRigSettings>,
            Has<AlsControlRigInput>,
            Option<&AlsFootTargetSelection>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, settings, has_input, selection) in &characters {
        let settings = settings.copied().unwrap_or_default();
        if !has_input {
            commands.entity(entity).insert(AlsControlRigInput {
                use_hand_ik_bones: settings.use_hand_ik_bones,
                use_foot_ik_bones: settings.use_foot_ik_bones,
                ..default()
            });
        }
        if selection.is_none_or(|selection| selection.use_ik_bones != settings.use_foot_ik_bones) {
            commands.entity(entity).insert(AlsFootTargetSelection {
                use_ik_bones: settings.use_foot_ik_bones,
            });
        }
    }
}

fn source_pelvis_offset_amount(player: &AnimationGraphPlayer, ground_prediction: f32) -> f32 {
    let amount = |field| {
        player
            .get_global_input_data(field)
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(0.0)
    };
    (amount("pose_state.grounded_amount") + amount("pose_state.in_air_amount") * ground_prediction)
        .clamp(0.0, 1.0)
}

#[allow(clippy::type_complexity)]
fn refresh_control_rig_input(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &AlsCharacterAssembly,
            &RotationMode,
            Has<AlsFirstPersonView>,
            Option<&AlsControlRigSettings>,
            Option<&AlsFootLockState>,
            Option<&AlsActualMotion>,
            Option<&mut AlsControlRigState>,
            Option<&mut AlsControlRigInput>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    for (
        entity,
        assembly,
        rotation_mode,
        first_person,
        settings,
        foot_lock,
        actual_motion,
        current_state,
        current_input,
    ) in &mut characters
    {
        let Some((player, context)) = avatars
            .get(assembly.avatar)
            .ok()
            .filter(|(avatar, _)| avatar.character == entity)
            .and_then(|(_, instance)| players.get(instance.player_entity()).ok())
        else {
            continue;
        };
        let scalar = |key| {
            player
                .get_global_input_data(key)
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(0.0)
        };
        let mut state = current_state.as_deref().copied().unwrap_or_default();
        let current_base = actual_motion.and_then(|motion| motion.movement_base_snapshot);
        let movement_base_delta_yaw = match (state.movement_base_snapshot, current_base) {
            (Some((previous_entity, previous_rotation)), Some((entity, rotation)))
                if previous_entity == entity =>
            {
                -world_forward_to_yaw(rotation * previous_rotation.conjugate() * Vec3::NEG_Z)
                    .to_degrees()
            }
            _ => 0.0,
        };
        state.movement_base_snapshot = current_base;
        update_spine_state(
            &mut state,
            SpineInput {
                allowed: *rotation_mode == RotationMode::Aiming || first_person,
                pending_update: player
                    .get_global_input_data("animation.pending_update")
                    .and_then(|value| value.as_bool().ok())
                    .unwrap_or(true),
                view_yaw_angle: scalar("view_state.yaw_angle"),
                view_yaw_speed: scalar("view_state.yaw_speed"),
                body_yaw_world: scalar("locomotion_state.rotation_yaw"),
                movement_base_delta_yaw,
                blend_amount: (1.0 - output_f32(context, "ViewBlock").clamp(0.0, 1.0))
                    * output_f32(context, "PoseAiming").clamp(0.0, 1.0),
                delta_seconds: scalar("animation.delta_seconds"),
            },
        );
        if let Some(mut existing) = current_state {
            *existing = state;
        } else {
            commands.entity(entity).insert(state);
        }
        let Some(mut current_input) = current_input else {
            continue;
        };
        let source_scalar = |key: &str| {
            source_player_property(player, key)
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(0.0)
        };
        let pelvis_offset_amount = source_pelvis_offset_amount(
            player,
            source_scalar("in_air_state.ground_prediction_amount"),
        );
        let velocity_blend = ["forward_amount", "backward_amount"]
            .map(|direction| source_scalar(&format!("grounded_state.velocity_blend.{direction}")));
        let settings = settings.copied().unwrap_or_default();
        let input = AlsControlRigInput {
            use_hand_ik_bones: settings.use_hand_ik_bones,
            use_foot_ik_bones: settings.use_foot_ik_bones,
            foot_transforms_valid: foot_lock.is_some_and(|state| state.valid)
                && current_input.foot_transforms_valid,
            velocity_blend_forward_amount: velocity_blend[0],
            velocity_blend_backward_amount: velocity_blend[1],
            spine_yaw_angle_degrees: state.final_yaw_angle,
            pelvis_offset_amount,
            ..*current_input
        };
        *current_input = input;
    }
}

#[doc(hidden)]
pub fn publish_control_rig_inputs(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    characters: Query<
        (
            Entity,
            &AlsControlRigInput,
            Option<&AlsControlRigFootLocationsCm>,
            Option<&AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
    transforms: TransformHelper,
    parents: Query<&ChildOf>,
    trees: Option<Res<ColliderTrees>>,
    colliders: Query<(
        Entity,
        &Position,
        &Rotation,
        &Collider,
        Option<&CollisionLayers>,
        Option<&crate::extensions::ComplexCollider>,
        Option<&ShapeCollisionMode>,
        Option<&CookedMeshRayData>,
    )>,
    capture: Option<Res<crate::foot_ik_apply::AlsFootIkTelemetryEnabled>>,
) {
    let mut world: Option<Arc<RigQueryWorld>> = None;
    for (entity, input, foot_locations, assembly) in &characters {
        let Some(assembly) = assembly else {
            continue;
        };
        let avatar = assembly.avatar;
        let Ok((owner, instance)) = avatars.get(avatar) else {
            continue;
        };
        if owner.character != entity {
            continue;
        }
        let root = instance.player_entity();
        let Ok(mut player) = players.get_mut(root) else {
            continue;
        };
        let mut ignored = vec![entity];
        if let Some(ragdoll) = player.get_spawned_ragdoll() {
            ignored.extend(ragdoll.bodies.values().copied());
            ignored.extend(ragdoll.colliders.values().copied());
        }
        let filter = SpatialQueryFilter::from_excluded_entities(ignored)
            .with_mask(crate::als::character::AlsCollisionLayer::Visibility);
        use bevy_animation_graph::core::edge_data::DataValue;
        for (field, value) in [
            (
                "b_use_hand_ik_bones",
                DataValue::Bool(input.use_hand_ik_bones),
            ),
            (
                "b_use_foot_ik_bones",
                DataValue::Bool(input.use_foot_ik_bones),
            ),
            (
                "b_foot_transforms_valid",
                DataValue::Bool(input.foot_transforms_valid),
            ),
            (
                "velocity_blend_forward_amount",
                DataValue::F32(input.velocity_blend_forward_amount),
            ),
            (
                "velocity_blend_backward_amount",
                DataValue::F32(input.velocity_blend_backward_amount),
            ),
            (
                "spine_yaw_angle",
                DataValue::F32(-input.spine_yaw_angle_degrees),
            ),
            (
                "pelvis_offset_amount",
                DataValue::F32(input.pelvis_offset_amount),
            ),
            (
                "foot_left_location",
                DataValue::Vec3(input.foot_left_location_armature),
            ),
            (
                "foot_left_rotation",
                DataValue::Quat(input.foot_left_rotation_armature),
            ),
            (
                "foot_right_location",
                DataValue::Vec3(input.foot_right_location_armature),
            ),
            (
                "foot_right_rotation",
                DataValue::Quat(input.foot_right_rotation_armature),
            ),
        ] {
            set_source_frame_input(&mut player, &format!("control_rig_input.{field}"), value);
        }
        set_source_frame_input(
            &mut player,
            crate::extensions::root_graph_boundaries::RIG_GAME_WORLD_KEY,
            true,
        );
        set_source_frame_input(
            &mut player,
            crate::extensions::root_graph_boundaries::RIG_CAPTURE_KEY,
            capture.as_ref().is_some_and(|capture| capture.0),
        );
        let mesh = character_mesh_entity(avatar, root, &parents);
        let context = if input.foot_transforms_valid {
            trees.as_ref().and_then(|_| {
                transforms
                    .compute_global_transform(mesh)
                    .ok()
                    .map(|transform| {
                        let world = world
                            .get_or_insert_with(|| {
                                let mut mesh_ray_data =
                                    bevy::platform::collections::HashMap::default();
                                let colliders = colliders
                                    .iter()
                                    .filter(|(_, _, _, _, _, _, mode, _)| {
                                        mode.is_none_or(|mode| mode.has_query())
                                    })
                                    .map(
                                        |(
                                            entity,
                                            position,
                                            rotation,
                                            collider,
                                            layers,
                                            complex,
                                            _,
                                            ray,
                                        )| {
                                            let mut surface =
                                                complex.map_or(collider, |value| &value.0).clone();
                                            surface.set_scale(collider.scale(), 10);
                                            if let Some(ray) = ray {
                                                mesh_ray_data.insert(entity, ray.clone());
                                            }
                                            (
                                                entity,
                                                (
                                                    *position,
                                                    *rotation,
                                                    surface,
                                                    layers.copied().unwrap_or_default(),
                                                ),
                                            )
                                        },
                                    )
                                    .collect();
                                Arc::new(RigQueryWorld {
                                    colliders,
                                    mesh_ray_data,
                                })
                            })
                            .clone();
                        let transform = transform.compute_transform();
                        RigQueryContext {
                            world,
                            filter,
                            channel: "ECC_Visibility".to_owned(),
                            to_world: RigTransform {
                                translation: transform.translation.as_dvec3(),
                                rotation: transform.rotation.as_dquat(),
                                scale: transform.scale.as_dvec3(),
                            },
                        }
                    })
            })
        } else {
            None
        };
        let mut source = source_inputs.get_mut(root).unwrap();
        source.set_rig_query_context(context);
        source.set_rig_foot_locations_cm(
            foot_locations
                .filter(|locations| {
                    input.foot_transforms_valid
                        && locations.0.map(|point| (point.as_dvec3() * 0.01).as_vec3())
                            == [
                                input.foot_left_location_armature,
                                input.foot_right_location_armature,
                            ]
                })
                .map(|locations| locations.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::motion::AlsViewYaw;
    use crate::als::state::LocomotionState;
    use crate::locomotion_layer::AlsGroundPredictionState;

    #[test]
    fn rig_input_snapshots_prepared_actor_state_before_source_update() {
        use crate::extensions::{
            post_physics_animation::PostPhysicsAnimationSet,
            source_instance::set_source_frame_input,
        };
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

        fn source_update(
            mut players: Query<(&mut AnimationGraphPlayer, &mut AlsRootAnimationContext)>,
        ) {
            for (mut player, mut context) in &mut players {
                for (key, value) in [
                    ("grounded_state.velocity_blend.forward_amount", 0.9),
                    ("grounded_state.velocity_blend.backward_amount", 0.05),
                    ("in_air_state.ground_prediction_amount", 0.8),
                ] {
                    set_source_frame_input(&mut player, key, value);
                }
                context.curves.insert("ViewBlock".into(), 1.0);
                context.curves.insert("PoseAiming".into(), 0.0);
            }
        }

        let mut app = App::new();
        app.add_plugins(crate::extensions::SourceAnimationInputsPlugin);
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ))
        .init_resource::<ColliderTrees>();
        let actor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                RotationMode::Aiming,
                Transform::default(),
                AlsViewYaw(60.0_f32.to_radians()),
                AlsFootLockState {
                    valid: true,
                    ..default()
                },
                AlsGroundPredictionState { amount: 0.95 },
                AlsControlRigInput {
                    foot_transforms_valid: true,
                    foot_left_location_armature: Vec3::new(1.0, 2.0, 3.0),
                    ..default()
                },
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character: actor },
                AnimatedSceneHandle::default(),
                ChildOf(actor),
            ))
            .id();
        app.world_mut()
            .entity_mut(actor)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });
        let avatar_transform = Transform::from_xyz(4.0, 1.0, 2.0)
            .with_rotation(Quat::from_rotation_y(0.4))
            .with_scale(Vec3::new(0.5, 2.0, 3.0));
        app.world_mut().entity_mut(avatar).insert(avatar_transform);
        let player = app
            .world_mut()
            .spawn((
                AnimationGraphPlayer::default(),
                AlsRootAnimationContext::default(),
                Transform::default(),
                ChildOf(avatar),
            ))
            .id();
        let mut parent = avatar;
        let spine = std::array::from_fn::<_, 4, _>(|_| {
            let bone = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    GlobalTransform::default(),
                    ChildOf(parent),
                ))
                .id();
            parent = bone;
            Some(bone)
        });
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity(),
            player
        );
        app.world_mut()
            .resource_mut::<bevy::ecs::schedule::Schedules>()
            .remove(PostUpdate);
        app.add_plugins(ControlRigApplyPlugin)
            .configure_sets(
                PostUpdate,
                PostPhysicsAnimationSet::Evaluate.before(AnimationGraphSet::Final),
            )
            .add_systems(
                PostUpdate,
                source_update.in_set(PostPhysicsAnimationSet::Evaluate),
            );
        let unloaded = app
            .world_mut()
            .spawn((
                AlsCharacter,
                AlsControlRigSettings {
                    use_hand_ik_bones: false,
                    use_foot_ik_bones: false,
                },
            ))
            .id();
        for (locomotion, mode, first_person, view_angle) in [
            (
                LocomotionState::Grounded,
                RotationMode::Aiming,
                false,
                -60.0,
            ),
            (LocomotionState::None, RotationMode::Aiming, false, -35.0),
            (
                LocomotionState::Grounded,
                RotationMode::ViewDirection,
                true,
                45.0,
            ),
        ] {
            app.world_mut().entity_mut(actor).insert((locomotion, mode));
            for bone in spine.into_iter().flatten() {
                app.world_mut()
                    .entity_mut(bone)
                    .insert((Transform::default(), GlobalTransform::default()));
            }
            if first_person {
                app.world_mut().entity_mut(actor).insert(AlsFirstPersonView);
            }
            {
                let mut player = app
                    .world_mut()
                    .get_mut::<AnimationGraphPlayer>(player)
                    .unwrap();
                for (key, value) in [
                    ("grounded_state.velocity_blend.forward_amount", 0.25),
                    ("grounded_state.velocity_blend.backward_amount", 0.1),
                    ("in_air_state.ground_prediction_amount", 0.2),
                ] {
                    set_source_frame_input(&mut player, key, value);
                }
                for (key, value) in [
                    ("pose_state.grounded_amount", 0.25),
                    ("pose_state.in_air_amount", 0.75),
                    ("view_state.yaw_angle", view_angle),
                    ("view_state.yaw_speed", 120.0),
                    ("locomotion_state.rotation_yaw", -10.0),
                ] {
                    player.set_global_input_data(key, value);
                }
                player.set_global_input_data("animation.pending_update", true);
            }
            app.world_mut()
                .get_mut::<AlsRootAnimationContext>(player)
                .unwrap()
                .curves = [("ViewBlock".into(), 0.25), ("PoseAiming".into(), 0.5)]
                .into_iter()
                .collect();
            app.world_mut().run_schedule(PostUpdate);
            let input = app.world().get::<AlsControlRigInput>(actor).unwrap();
            assert_eq!(
                (
                    input.velocity_blend_forward_amount,
                    input.velocity_blend_backward_amount
                ),
                (0.25, 0.1)
            );
            assert!((input.pelvis_offset_amount - 0.4).abs() < 1e-6, "{input:?}");
            assert!(
                (input.spine_yaw_angle_degrees - view_angle * 0.375).abs() < 1e-6,
                "{input:?}"
            );
            assert_eq!(input.foot_left_location_armature, Vec3::new(1.0, 2.0, 3.0));
            assert!(input.foot_transforms_valid);
            let published = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert_eq!(
                source_player_property(
                    published,
                    "control_rig_input.velocity_blend_forward_amount"
                )
                .and_then(|value| value.as_f32().ok()),
                Some(0.25)
            );
            assert_eq!(
                source_player_property(published, "control_rig_input.foot_left_location")
                    .and_then(|value| value.as_vec3().ok()),
                Some(Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                source_player_property(published, "control_rig_input.spine_yaw_angle")
                    .and_then(|value| value.as_f32().ok()),
                Some(-input.spine_yaw_angle_degrees)
            );
            let query = app
                .world()
                .get::<crate::extensions::SourceAnimationInputs>(player)
                .unwrap()
                .rig_query_context()
                .unwrap();
            let avatar_world = GlobalTransform::from(avatar_transform).compute_transform();
            assert_eq!(
                query.to_world.translation,
                avatar_world.translation.as_dvec3()
            );
            assert_eq!(query.to_world.rotation, avatar_world.rotation.as_dquat());
            assert_eq!(query.to_world.scale, avatar_world.scale.as_dvec3());
            assert!(query.filter.excluded_entities.contains(&actor));
            assert!(
                !query.filter.excluded_entities.contains(&unloaded),
                "Other characters must remain traceable"
            );
            assert!(!query.filter.test(
                Entity::PLACEHOLDER,
                avian3d::prelude::CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Default,
                    avian3d::prelude::LayerMask::ALL
                )
            ));
            assert!(query.filter.test(
                Entity::PLACEHOLDER,
                avian3d::prelude::CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Visibility,
                    avian3d::prelude::LayerMask::ALL
                )
            ));
            assert_eq!(
                app.world()
                    .get::<AlsControlRigState>(actor)
                    .unwrap()
                    .final_yaw_angle,
                input.spine_yaw_angle_degrees
            );
            let expected = Quat::IDENTITY;
            let actual = app
                .world()
                .get::<Transform>(spine[0].unwrap())
                .unwrap()
                .rotation;
            assert!(
                actual.abs_diff_eq(expected, 1e-6),
                "{locomotion:?}: {actual:?} != {expected:?}"
            );
        }
        assert!(
            app.world()
                .get::<AlsFootTargetSelection>(actor)
                .unwrap()
                .use_ik_bones
        );
        for use_foot_ik_bones in [false, true] {
            app.world_mut()
                .entity_mut(actor)
                .insert(AlsControlRigSettings {
                    use_hand_ik_bones: true,
                    use_foot_ik_bones,
                });
            app.world_mut().run_schedule(PostUpdate);
            assert_eq!(
                app.world()
                    .get::<AlsControlRigInput>(actor)
                    .unwrap()
                    .use_foot_ik_bones,
                use_foot_ik_bones
            );
            assert_eq!(
                app.world()
                    .get::<AlsFootTargetSelection>(actor)
                    .unwrap()
                    .use_ik_bones,
                use_foot_ik_bones
            );
        }

        let actor_transform = *app.world().get::<Transform>(actor).unwrap();
        let spawn_transform = Transform::from_xyz(-15.0, 0.919_999_96, -15.0);
        let default_mount = crate::character_assembly::avatar_transform();
        app.world_mut().entity_mut(actor).insert(spawn_transform);
        app.world_mut().entity_mut(avatar).insert(default_mount);
        app.world_mut().run_schedule(PostUpdate);
        let query = app
            .world()
            .get::<crate::extensions::SourceAnimationInputs>(player)
            .unwrap()
            .rig_query_context()
            .unwrap();
        let mesh_world = GlobalTransform::from(spawn_transform).mul_transform(default_mount);
        assert_eq!(
            query.to_world.translation,
            mesh_world.translation().as_dvec3()
        );
        let left = bevy::math::DVec3::new(
            17.076_301_574_707_03,
            13.465_350_151_062_012,
            -5.072_169_780_731_201,
        );
        let right = bevy::math::DVec3::new(
            -17.076_314_926_147_46,
            13.465_192_794_799_805,
            -5.072_188_854_217_529,
        );
        app.world_mut()
            .entity_mut(actor)
            .insert(AlsControlRigFootLocationsCm([
                left.as_vec3(),
                right.as_vec3(),
            ]));
        {
            let mut input = app
                .world_mut()
                .get_mut::<AlsControlRigInput>(actor)
                .unwrap();
            input.foot_left_location_armature = (left * 0.01).as_vec3();
            input.foot_right_location_armature = (right * 0.01).as_vec3();
        }
        app.world_mut().run_schedule(PostUpdate);
        let source = app
            .world()
            .get::<crate::extensions::SourceAnimationInputs>(player)
            .unwrap();
        assert_eq!(
            source.rig_foot_locations_cm().unwrap().map(Vec3::as_dvec3),
            [left, right]
        );
        app.world_mut()
            .get_mut::<AlsControlRigInput>(actor)
            .unwrap()
            .foot_left_location_armature = Vec3::X;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            app.world()
                .get::<crate::extensions::SourceAnimationInputs>(player)
                .unwrap()
                .rig_foot_locations_cm()
                .is_none()
        );
        app.world_mut().entity_mut(actor).insert(actor_transform);
        app.world_mut().entity_mut(avatar).insert(avatar_transform);

        let initialized = app.world().get::<AlsControlRigInput>(unloaded).unwrap();
        assert!(
            !initialized.use_hand_ik_bones
                && !initialized.use_foot_ik_bones
                && !initialized.foot_transforms_valid
        );
        assert!(
            !app.world()
                .get::<AlsFootTargetSelection>(unloaded)
                .unwrap()
                .use_ik_bones
        );
        assert!(
            app.world()
                .get::<AlsControlRigExecution>(unloaded)
                .is_none()
        );
        app.world_mut()
            .get_mut::<AlsControlRigInput>(unloaded)
            .unwrap()
            .pelvis_offset_amount = 0.75;
        app.world_mut()
            .get_mut::<AlsControlRigInput>(actor)
            .unwrap()
            .foot_transforms_valid = false;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            app.world()
                .get::<crate::extensions::SourceAnimationInputs>(player)
                .unwrap()
                .rig_query_context()
                .is_none()
        );
        assert_eq!(
            app.world()
                .get::<AlsControlRigInput>(unloaded)
                .unwrap()
                .pelvis_offset_amount,
            0.75
        );

        let base = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(actor)
            .remove::<AlsFirstPersonView>()
            .insert((
                RotationMode::ViewDirection,
                AlsActualMotion {
                    movement_base_snapshot: Some((
                        base,
                        Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
                    )),
                    ..default()
                },
                AlsControlRigState {
                    spine_amount: 1.0,
                    last_yaw_angle: 20.0,
                    last_yaw_angle_world_space: 10.0,
                    movement_base_snapshot: Some((base, Quat::IDENTITY)),
                    ..default()
                },
            ));
        {
            let mut player = app
                .world_mut()
                .get_mut::<AnimationGraphPlayer>(player)
                .unwrap();
            player.set_global_input_data("animation.pending_update", false);
            player.set_global_input_data("locomotion_state.rotation_yaw", 10.0_f32);
        }
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            (app.world()
                .get::<AlsControlRigState>(actor)
                .unwrap()
                .last_yaw_angle_world_space
                + 20.0)
                .abs()
                < 1e-5
        );
        let next_base = app.world_mut().spawn_empty().id();
        app.world_mut()
            .get_mut::<AlsActualMotion>(actor)
            .unwrap()
            .movement_base_snapshot = Some((
            next_base,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        ));
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            (app.world()
                .get::<AlsControlRigState>(actor)
                .unwrap()
                .last_yaw_angle_world_space
                + 20.0)
                .abs()
                < 1e-5
        );
        {
            let mut state = app
                .world_mut()
                .get_mut::<AlsControlRigState>(actor)
                .unwrap();
            state.spine_amount = 1.0;
            state.last_yaw_angle_world_space = 10.0;
        }
        app.world_mut()
            .get_mut::<AlsActualMotion>(actor)
            .unwrap()
            .movement_base_snapshot = Some((
            next_base,
            Quat::from_rotation_y(3.0 * std::f32::consts::FRAC_PI_4),
        ));
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            (app.world()
                .get::<AlsControlRigState>(actor)
                .unwrap()
                .last_yaw_angle_world_space
                + 20.0)
                .abs()
                < 1e-5
        );
        app.world_mut()
            .get_mut::<AlsControlRigState>(actor)
            .unwrap()
            .last_yaw_angle_world_space = 10.0;
        app.world_mut()
            .get_mut::<AlsActualMotion>(actor)
            .unwrap()
            .movement_base_snapshot = None;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            (app.world()
                .get::<AlsControlRigState>(actor)
                .unwrap()
                .last_yaw_angle_world_space
                - 10.0)
                .abs()
                < 1e-5
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        app.world_mut()
            .get_mut::<AlsRootAnimationContext>(player)
            .unwrap()
            .curves = [("ViewBlock".into(), 0.0), ("PoseAiming".into(), 1.0)]
            .into_iter()
            .collect();
        app.world_mut()
            .entity_mut(actor)
            .insert(AlsControlRigState {
                spine_rotation_allowed: true,
                spine_amount: 1.0,
                yaw_angle: 60.0,
                ..default()
            });
        {
            let mut player = app
                .world_mut()
                .get_mut::<AnimationGraphPlayer>(player)
                .unwrap();
            player.set_global_input_data("animation.delta_seconds", 0.05_f32);
            player.set_global_input_data("view_state.yaw_speed", 0.0_f32);
            player.set_global_input_data("locomotion_state.rotation_yaw", 0.0_f32);
        }
        app.world_mut().run_schedule(PostUpdate);
        let x = std::f32::consts::LN_2 / (0.7 + 1e-8) * 0.05;
        let source_amount =
            1.0 / (1.0 + 1.00746054 * x + 0.45053901 * x * x + 0.25724632 * x * x * x);
        let input = app.world().get::<AlsControlRigInput>(actor).unwrap();
        assert!(
            (input.spine_yaw_angle_degrees - 60.0 * source_amount).abs() < 1e-5,
            "Spine must use the actor animation delta: {input:?}, source amount {source_amount}"
        );
        let published = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert!(
            (source_player_property(published, "control_rig_input.spine_yaw_angle")
                .and_then(|value| value.as_f32().ok())
                .unwrap()
                + 60.0 * source_amount)
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn spine_interpolation_uses_source_counterclockwise_and_weight_boundaries() {
        assert!((lerp_angle(0.0, 179.0, 0.5) + 90.5).abs() < 1e-6);
        assert!((lerp_angle(0.0, 175.0, 0.5) - 87.5).abs() < 1e-6);
        assert!(is_relevant(0.000_02));
        assert!(!is_full(0.999_98));
    }

    #[test]
    fn pelvis_offset_reads_the_prepared_actor_pose_inputs() {
        let mut player = AnimationGraphPlayer::default();
        for (grounded, in_air, prediction, expected) in [
            (1.0_f32, 0.0_f32, 0.0_f32, 1.0_f32),
            (0.25, 0.75, 0.4, 0.55),
            (0.0, 1.0, 0.0, 0.0),
        ] {
            player.set_global_input_data("pose_state.grounded_amount", grounded);
            player.set_global_input_data("pose_state.in_air_amount", in_air);
            assert!((source_pelvis_offset_amount(&player, prediction) - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn spine_uses_the_actor_pending_update_and_view_speed() {
        let mut state = AlsControlRigState::default();
        let mut input = SpineInput {
            allowed: true,
            pending_update: false,
            view_yaw_angle: 80.0,
            view_yaw_speed: 0.0,
            body_yaw_world: 0.0,
            movement_base_delta_yaw: 0.0,
            blend_amount: 0.25,
            delta_seconds: 0.1,
        };
        update_spine_state(&mut state, input);
        assert!(state.spine_amount > 0.49 && state.spine_amount < 0.51);
        input.pending_update = true;
        update_spine_state(&mut state, input);
        assert!((state.spine_amount - 1.0).abs() < 1e-6);
        assert!((state.yaw_angle - 80.0).abs() < 1e-6);
        assert!((state.final_yaw_angle - 20.0).abs() < 1e-6);
        input.allowed = false;
        input.pending_update = false;
        input.view_yaw_speed = 3580.0;
        update_spine_state(&mut state, input);
        let x = std::f32::consts::LN_2 / (0.7 * 40.0 / 3580.0 + 1e-8) * 0.1;
        let expected = 1.0 / (1.0 + 1.00746054 * x + 0.45053901 * x * x + 0.25724632 * x * x * x);
        assert!((state.spine_amount - expected).abs() < 1e-6);
        assert!((state.final_yaw_angle - 20.0 * expected).abs() < 1e-5);
        input.pending_update = true;
        update_spine_state(&mut state, input);
        assert_eq!(
            (state.spine_amount, state.yaw_angle, state.final_yaw_angle),
            (0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn spine_exit_tracks_rotating_base_before_world_yaw_clamp() {
        let mut state = AlsControlRigState {
            spine_amount: 1.0,
            last_yaw_angle: 20.0,
            last_yaw_angle_world_space: 10.0,
            ..default()
        };
        update_spine_state(
            &mut state,
            SpineInput {
                allowed: false,
                pending_update: false,
                view_yaw_angle: 0.0,
                view_yaw_speed: 0.0,
                body_yaw_world: 10.0,
                movement_base_delta_yaw: 45.0,
                blend_amount: 1.0,
                delta_seconds: 0.1,
            },
        );
        assert!((state.last_yaw_angle_world_space - 40.0).abs() < 1e-5);
        assert!((state.yaw_angle - 50.0 * state.spine_amount).abs() < 1e-5);
    }
}
