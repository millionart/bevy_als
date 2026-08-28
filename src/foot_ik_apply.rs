use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets,
    AlsFootLockSettings, AlsFootProbe, FootSideInput, constrain_foot_lock_location,
    foot_target_offset,
};
use als_core::state::Stance;
use als_core::transitions::TransitionConfig;
use als_ext::ik::{solve_two_bone_ik, world_rotation_to_local};
use avian3d::prelude::LinearVelocity;
use bevy::animation::AnimationPlayer;
use bevy::prelude::*;
use bevy::transform::{TransformSystems, systems as transform_systems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::apply_animation_to_targets;

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
            &LinearVelocity,
            &Stance,
            &AlsFootLockSettings,
            &mut AlsFootIkRuntime,
            &mut AlsDynamicTransitionState,
            Option<&AnimationPlayer>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    _scenes: Query<&AnimatedSceneInstance>,
) {
    let dt = time.delta_secs();
    for (
        _entity,
        char_gtf,
        bones,
        probe,
        targets,
        velocity,
        stance,
        settings,
        mut runtime,
        mut dynamic_transition,
        _,
    ) in &mut characters
    {
        let char_fwd = char_gtf.forward().as_vec3();
        let pole = Vec3::new(char_fwd.x, 0.0, char_fwd.z).normalize_or_zero();

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

        let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
        let moving_smooth = horizontal_speed >= settings.moving_smooth_speed_threshold;
        runtime.update_with_foot_lock(
            &inputs[0],
            &inputs[1],
            grounded_target(targets),
            settings,
            moving_smooth,
            targets.grounded,
            dt,
        );
        dynamic_transition.update(&runtime, *stance, &TransitionConfig::default());

        for side in 0..2usize {
            let (offset, pin_weight) = runtime.side(side);
            if offset.abs() < 1e-4 && pin_weight < 1e-3 {
                continue;
            }
            apply_chain_offset(
                &bones.chain(side),
                offset,
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

fn grounded_target(targets: &AlsFootIkTargets) -> f32 {
    if targets.grounded { 1.0 } else { 0.0 }
}

fn apply_chain_offset(
    chain: &als_core::foot_ik::FootChain,
    offset: f32,
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
        ankle + Vec3::Y * offset,
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
    let foot_parent_rotation = bone_globals
        .get(foot_child.0)
        .map(|parent| parent.rotation())
        .unwrap_or(Quat::IDENTITY);
    let (_, foot_pitch, foot_roll) = foot_g.rotation().to_euler(EulerRot::YXZ);
    let foot_world_rotation =
        Quat::from_euler(EulerRot::YXZ, constrained_foot_yaw, foot_pitch, foot_roll);
    foot_local.rotation = world_rotation_to_local(foot_parent_rotation, foot_world_rotation);
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
        pelvis_local.translation.y -= drop;
    }
}
