use als_core::overlay_blend::{BLEND_IN_DURATION, BLEND_OUT_DURATION, OverlayBlendState};
use als_core::state::OverlayState;
use als_ext::{
    AlsAnimCurves, AlsPoseCurves, HEAD_LAYER_BONES, LOOK_SPINE_BONES, TORCH_ARM_RIGHT_BONES,
    TORCH_SPINE_UPPER_BONES, build_bone_mask, build_grouped_bone_mask,
};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::skeleton::Skeleton;

use crate::character_assembly::AlsCharacterAvatar;

pub const OVERLAY_CURVES_ASSET: &str = "animations/Overlays/Torch/A_Als_Torch_Poses.gltf#AlsCurves";
pub const SKELETON_ASSET: &str = "skeletons/SKM_Als.skn.ron";

pub struct OverlayLayerPlugin;

impl Plugin for OverlayLayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverlayLayerAssets>()
            .add_systems(Startup, load_overlay_assets)
            .add_systems(Update, overlay_layer_driver);
    }
}

#[derive(Resource, Default)]
pub struct OverlayLayerAssets {
    pub curves: Option<Handle<AlsAnimCurves>>,
    pub skeleton: Option<Handle<Skeleton>>,
}

fn load_overlay_assets(mut assets: ResMut<OverlayLayerAssets>, server: Res<AssetServer>) {
    if assets.curves.is_none() {
        assets.curves = Some(server.load(OVERLAY_CURVES_ASSET));
    }
    if assets.skeleton.is_none() {
        assets.skeleton = Some(server.load(SKELETON_ASSET));
    }
}

fn overlay_target_on(overlay: &OverlayState) -> bool {
    *overlay != OverlayState::Default
}

fn ramp(t: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        1.0
    } else {
        (t / duration).clamp(0.0, 1.0)
    }
}

fn dynamic_upper_mask(
    curves: &AlsPoseCurves,
    skeleton: &Skeleton,
    elapsed: f32,
) -> bevy_animation_graph::core::edge_data::bone_mask::BoneMask {
    let spine_w = curves.sample("LayerSpineAdditive", elapsed).unwrap_or(1.0);
    let arm_w = curves.sample("LayerArmRight", elapsed).unwrap_or(1.0);
    let head_w = curves.sample("LayerHeadAdditive", elapsed).unwrap_or(0.0);
    build_grouped_bone_mask(
        skeleton,
        &[
            (TORCH_SPINE_UPPER_BONES, spine_w),
            (TORCH_ARM_RIGHT_BONES, arm_w),
            (HEAD_LAYER_BONES, head_w),
        ],
    )
}

type OverlayDriverQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ChildOf,
        Option<&'static mut OverlayBlendState>,
        &'static AnimatedSceneInstance,
    ),
    With<AlsCharacterAvatar>,
>;

fn overlay_layer_driver(
    mut commands: Commands,
    time: Res<Time>,
    layer_assets: Res<OverlayLayerAssets>,
    curve_assets: Res<Assets<AlsAnimCurves>>,
    skeletons: Res<Assets<Skeleton>>,
    overlays: Query<&OverlayState, With<als_core::character::AlsCharacter>>,
    mut q: OverlayDriverQuery,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let Some(curves_handle) = layer_assets.curves.as_ref() else {
        return;
    };
    let Some(skeleton_handle) = layer_assets.skeleton.as_ref() else {
        return;
    };
    let Some(curves) = curve_assets.get(curves_handle) else {
        return;
    };
    let Some(skeleton) = skeletons.get(skeleton_handle) else {
        return;
    };
    let pose_curves = match curves.get("Torch_Poses") {
        Some(c) => c,
        None => match curves.first() {
            Some(c) => c,
            None => return,
        },
    };
    let look_mask = build_bone_mask(skeleton, LOOK_SPINE_BONES, 1.0);
    let dt = time.delta_secs();

    for (entity, child_of, blend, instance) in &mut q {
        let Ok(overlay) = overlays.get(child_of.parent()) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let target_on = overlay_target_on(overlay);
        let weight = match blend {
            Some(mut state) => {
                let t = state.phase_time;
                state.update(
                    target_on,
                    dt,
                    ramp(t, BLEND_IN_DURATION),
                    ramp(t, BLEND_OUT_DURATION),
                )
            }
            None => {
                commands.entity(entity).insert(OverlayBlendState::default());
                0.0
            }
        };
        player.set_input_data("overlay_weight", DataValue::from(weight));

        let elapsed = player.elapsed();
        let upper = dynamic_upper_mask(pose_curves, skeleton, elapsed);
        player.set_input_data("upper_body_mask", DataValue::BoneMask(upper));
        player.set_input_data("look_mask", DataValue::BoneMask(look_mask.clone()));
    }
}
