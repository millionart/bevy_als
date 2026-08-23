use als_core::foot_ik::{
    foot_target_offset, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets, AlsFootProbe,
    FootSideInput,
};
use bevy::animation::AnimationPlayer;
use bevy::prelude::*;
use bevy::transform::{systems as transform_systems, TransformSystems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::systems::apply_animation_to_targets;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use als_ext::ik::{solve_two_bone_ik, world_rotation_to_local};

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
            &mut AlsFootIkRuntime,
            Option<&AnimationPlayer>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    _scenes: Query<&AnimatedSceneInstance>,
) {
    let dt = time.delta_secs();
    for (_entity, char_gtf, bones, probe, targets, mut runtime, _) in &mut characters {
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
                    raw_offset: hit
                        .map(|h| foot_target_offset(ankle_y, &h))
                        .unwrap_or(0.0),
                    pin_request: match side {
                        0 => targets.pin_left,
                        _ => targets.pin_right,
                    },
                };
            }
        }

        runtime.update(&inputs[0], &inputs[1], grounded_target(targets), dt);

        for side in 0..2usize {
            let (offset, _pin_w) = runtime.side(side);
            if offset.abs() < 1e-4 {
                continue;
            }
            apply_chain_offset(
                &bones.chain(side),
                offset,
                pole,
                &mut bone_locals,
                &bone_globals,
            );
        }

        apply_pelvis_drop(bones.left_thigh, runtime.pelvis_drop, &mut bone_locals);
    }
}

fn grounded_target(targets: &AlsFootIkTargets) -> f32 {
    if targets.grounded {
        1.0
    } else {
        0.0
    }
}

fn apply_chain_offset(
    chain: &als_core::foot_ik::FootChain,
    offset: f32,
    pole: Vec3,
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

    let target = ankle + Vec3::Y * offset;

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
