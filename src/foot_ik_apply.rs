use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets, AlsFootLockInput,
    AlsFootLockMovementBase, AlsFootLockSettings, AlsFootLockState, AlsFootProbe, FootSideInput,
    advance_foot_lock_movement_base, capture_foot_lock_movement_base, constrain_foot_lock_location,
    foot_target_offset, refresh_foot_lock,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transitions::TransitionConfig;
use als_ext::ik::{solve_two_bone_ik, world_rotation_to_local};
use bevy::animation::AnimationPlayer;
use bevy::prelude::*;
use bevy::transform::{TransformSystems, systems as transform_systems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::apply_animation_to_targets;

use crate::character_assembly::AlsCharacterAvatar;

pub struct FootIkApplyPlugin;

impl Plugin for FootIkApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AnimationGraphSet::Final.before(TransformSystems::Propagate),
        );
        app.add_systems(
            PostUpdate,
            (
                ensure_foot_ik_runtime,
                ensure_foot_lock_components,
                foot_ik_apply_system
                    .after(apply_animation_to_targets)
                    .after(TransformSystems::Propagate)
                    .after(als_core::foot_ik::foot_ground_probe_system)
                    .after(als_core::foot_ik::foot_pin_request_system)
                    .after(als_core::foot_ik::foot_ik_state_system),
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .after(foot_ik_apply_system)
                    .after(TransformSystems::Propagate),
            ),
        );
    }
}

fn ensure_foot_lock_components(
    mut commands: Commands,
    states: Query<Entity, (With<AlsFootBones>, Without<AlsFootLockState>)>,
    inputs: Query<Entity, (With<AlsFootBones>, Without<AlsFootLockInput>)>,
) {
    for entity in &states {
        commands.entity(entity).insert(AlsFootLockState::default());
    }
    for entity in &inputs {
        commands.entity(entity).insert(AlsFootLockInput::default());
    }
}

fn ensure_foot_ik_runtime(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<AlsFootIkRuntime>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsFootIkRuntime::default());
    }
}

#[allow(clippy::type_complexity)]
fn foot_ik_apply_system(
    time: Res<Time>,
    mut characters: Query<
        (
            Entity,
            &GlobalTransform,
            &AlsFootBones,
            &AlsFootProbe,
            &AlsFootIkTargets,
            &LocomotionState,
            &AlsFootLockInput,
            &Stance,
            &AlsFootLockSettings,
            Option<&AlsMovementBaseDynamicsState>,
            &mut AlsFootIkRuntime,
            &mut AlsFootLockState,
            &mut AlsDynamicTransitionState,
            Option<&AnimationPlayer>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (
        entity,
        char_gtf,
        bones,
        probe,
        targets,
        locomotion,
        lock_input,
        stance,
        settings,
        movement_base,
        mut runtime,
        mut lock_state,
        mut dynamic_transition,
        _,
    ) in &mut characters
    {
        let char_fwd = char_gtf.forward().as_vec3();
        let pole = Vec3::new(char_fwd.x, 0.0, char_fwd.z).normalize_or_zero();
        let left_ik_amount = animation_curve_amount(entity, "FootLeftIk", &avatars, &players)
            .unwrap_or(lock_input.left_ik_amount)
            .clamp(0.0, 1.0);
        let right_ik_amount = animation_curve_amount(entity, "FootRightIk", &avatars, &players)
            .unwrap_or(lock_input.right_ik_amount)
            .clamp(0.0, 1.0);
        let left_lock_amount = animation_curve_amount(entity, "FootLeftLock", &avatars, &players)
            .unwrap_or(lock_input.left_lock_amount)
            .clamp(0.0, 1.0);
        let right_lock_amount = animation_curve_amount(entity, "FootRightLock", &avatars, &players)
            .unwrap_or(lock_input.right_lock_amount)
            .clamp(0.0, 1.0);
        lock_state.became_valid = !lock_state.valid;
        lock_state.valid = true;
        lock_state.foot_planted_amount = lock_input.foot_planted_amount.clamp(-1.0, 1.0);
        lock_state.feet_crossing_amount = lock_input.feet_crossing_amount.clamp(0.0, 1.0);
        let became_valid = lock_state.became_valid;
        let current_base = movement_base.and_then(|state| {
            state
                .entity
                .and_then(|entity| foot_lock_movement_base(entity, &bone_globals))
        });
        for side in 0..2usize {
            let Ok(target) = bone_globals.get(bones.target(side)) else {
                continue;
            };
            let (foot_state, ik_amount, lock_amount) = match side {
                0 => (&mut lock_state.left, left_ik_amount, left_lock_amount),
                _ => (&mut lock_state.right, right_ik_amount, right_lock_amount),
            };
            let latest_base = foot_state
                .movement_base
                .and_then(|base| foot_lock_movement_base(base.entity, &bone_globals));
            advance_foot_lock_movement_base(foot_state, latest_base);
            refresh_foot_lock(
                foot_state,
                target.translation(),
                target.rotation(),
                ik_amount,
                lock_amount,
                settings.allow_foot_lock,
                lock_input.moving_smooth,
                *locomotion == LocomotionState::Grounded,
                became_valid,
                dt,
            );
            capture_foot_lock_movement_base(foot_state, current_base);
        }

        let mut inputs = [FootSideInput::default(), FootSideInput::default()];
        for side in 0..2usize {
            let hit = match side {
                0 => probe.left,
                _ => probe.right,
            };
            let chain = bones.chain(side);
            if let Ok((_, foot_g, _)) = bone_locals.get(chain.foot) {
                let ankle_y = foot_g.translation().y;
                inputs[side] = FootSideInput {
                    raw_offset: hit.map(|h| foot_target_offset(ankle_y, &h)).unwrap_or(0.0),
                    pin_request: match side {
                        0 => targets.pin_left,
                        _ => targets.pin_right,
                    },
                };
            }
        }

        runtime.update_with_foot_lock_per_side(
            &inputs[0],
            &inputs[1],
            left_ik_amount,
            right_ik_amount,
            settings,
            lock_input.moving_smooth,
            targets.grounded,
            dt,
        );
        dynamic_transition.update(&runtime, *stance, &TransitionConfig::default());

        for side in 0..2usize {
            let (offset, pin_weight) = runtime.side(side);
            let lock_offset = match side {
                0 => lock_state.left.final_location - lock_state.left.target_location,
                _ => lock_state.right.final_location - lock_state.right.target_location,
            };
            if offset.abs() < 1e-4 && pin_weight < 1e-3 && lock_offset.length_squared() < 1e-8 {
                continue;
            }
            apply_chain_offset(
                &bones.chain(side),
                Vec3::Y * offset + lock_offset,
                pole,
                settings,
                runtime.constrained_foot_yaw(
                    side,
                    target_foot_yaw(&bones.chain(side), &bone_globals),
                    settings,
                ),
                &mut bone_locals,
                &bone_globals,
            );
        }

        apply_pelvis_drop(bones.left_thigh, runtime.pelvis_drop, &mut bone_locals);
    }
}

fn foot_lock_movement_base(
    entity: Entity,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<AlsFootLockMovementBase> {
    let transform = transforms.get(entity).ok()?;
    Some(AlsFootLockMovementBase {
        entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
    })
}

fn animation_curve_amount(
    character: Entity,
    name: &str,
    avatars: &Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: &Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
) -> Option<f32> {
    let (_, instance) = avatars
        .iter()
        .find(|(parent, _)| parent.parent() == character)?;
    let player = players.get(instance.player_entity()).ok()?;
    player
        .get_outputs()
        .get(name)
        .and_then(|value| match value {
            bevy_animation_graph::core::edge_data::DataValue::F32(value) => Some(*value),
            _ => None,
        })
}

fn apply_chain_offset(
    chain: &als_core::foot_ik::FootChain,
    offset: Vec3,
    pole: Vec3,
    settings: &AlsFootLockSettings,
    constrained_foot_yaw: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) {
    let (Ok(_), Ok(_), Ok(_)) = (
        bone_locals.get(chain.thigh),
        bone_locals.get(chain.calf),
        bone_locals.get(chain.foot),
    ) else {
        return;
    };
    let (Ok(thigh_g), Ok(calf_g), Ok(foot_g)) = (
        bone_globals.get(chain.thigh),
        bone_globals.get(chain.calf),
        bone_globals.get(chain.foot),
    ) else {
        return;
    };

    let hip = thigh_g.translation();
    let knee = calf_g.translation();
    let ankle = foot_g.translation();

    let upper_len = (knee - hip).length();
    let lower_len = (ankle - knee).length();

    let target = constrain_foot_lock_location(
        hip,
        thigh_g.forward().as_vec3(),
        ankle + offset,
        settings.thigh_angle_limit_degrees,
    );

    let Some(solve) = solve_two_bone_ik(hip, knee, ankle, target, upper_len, lower_len, pole)
    else {
        return;
    };

    let thigh_world_new = solve.hip_rotation_world * thigh_g.rotation();
    let calf_world_new = solve.knee_rotation_world * calf_g.rotation();

    let Ok((mut thigh_local, _, thigh_child)) = bone_locals.get_mut(chain.thigh) else {
        return;
    };
    let pelvis_rot = bone_globals
        .get(thigh_child.0)
        .map(|p| p.rotation())
        .unwrap_or(Quat::IDENTITY);
    thigh_local.rotation = world_rotation_to_local(pelvis_rot, thigh_world_new);

    let Ok((mut calf_local, _, _)) = bone_locals.get_mut(chain.calf) else {
        return;
    };
    calf_local.rotation = world_rotation_to_local(thigh_world_new, calf_world_new);

    let Ok((mut foot_local, _, foot_child)) = bone_locals.get_mut(chain.foot) else {
        return;
    };
    let foot_parent_rotation = if foot_child.0 == chain.calf {
        calf_world_new
    } else {
        bone_globals
            .get(foot_child.0)
            .map(|parent| parent.rotation())
            .unwrap_or(Quat::IDENTITY)
    };
    let (_, foot_pitch, foot_roll) = foot_g.rotation().to_euler(EulerRot::YXZ);
    let foot_world_rotation =
        Quat::from_euler(EulerRot::YXZ, constrained_foot_yaw, foot_pitch, foot_roll);
    foot_local.rotation = foot_local_rotation(foot_parent_rotation, foot_world_rotation);
}

fn foot_local_rotation(parent_world_rotation: Quat, foot_world_rotation: Quat) -> Quat {
    world_rotation_to_local(parent_world_rotation, foot_world_rotation)
}

fn target_foot_yaw(
    chain: &als_core::foot_ik::FootChain,
    bone_globals: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> f32 {
    bone_globals
        .get(chain.foot)
        .map(|transform| transform.rotation().to_euler(EulerRot::YXZ).0)
        .unwrap_or(0.0)
}

fn apply_pelvis_drop(
    thigh: Entity,
    drop: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
) {
    if drop.abs() < 1e-4 {
        return;
    }
    let pelvis_entity = match bone_locals.get(thigh) {
        Ok((_, _, c)) => c.0,
        Err(_) => return,
    };
    if let Ok((mut pelvis_local, _, _)) = bone_locals.get_mut(pelvis_entity) {
        pelvis_local.translation.y += drop;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foot_local_rotation_uses_the_updated_calf_parent() {
        let calf_old = Quat::from_rotation_y(30.0_f32.to_radians());
        let calf_new = Quat::from_rotation_y(50.0_f32.to_radians());
        let foot_target = Quat::IDENTITY;
        let corrected_local = foot_local_rotation(calf_new, foot_target);
        let legacy_local = foot_local_rotation(calf_old, foot_target);
        let corrected_world = calf_new * corrected_local;
        let legacy_world = calf_new * legacy_local;
        let corrected_alignment = corrected_world.dot(foot_target).abs();
        let legacy_alignment = legacy_world.dot(foot_target).abs();
        let passed = corrected_alignment > 0.99999 && legacy_alignment < 0.99;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.40][VERDICT] corrected_alignment={corrected_alignment:.6} legacy_alignment={legacy_alignment:.6} -> {verdict}"
        );
        assert!(passed);
    }
}
