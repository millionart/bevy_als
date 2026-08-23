use als_core::overlay_blend::OverlayBlendState;
use als_core::state::OverlayState;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use als_ext::CurveAsset;

pub struct OverlayLayerPlugin;

impl Plugin for OverlayLayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverlayLayerAssets>()
            .add_systems(Startup, load_overlay_curves)
            .add_systems(Update, overlay_layer_driver);
    }
}

#[derive(Resource, Default)]
pub struct OverlayLayerAssets {
    pub aim_in: Option<Handle<CurveAsset>>,
    pub aim_out: Option<Handle<CurveAsset>>,
}

fn load_overlay_curves(mut assets: ResMut<OverlayLayerAssets>, server: Res<AssetServer>) {
    if assets.aim_in.is_none() {
        assets.aim_in = Some(
            server.load("curves/ALS/Data/AnimationInstance/Blending/CF_Als_Aim_In.curve.ron"),
        );
    }
    if assets.aim_out.is_none() {
        assets.aim_out = Some(
            server.load("curves/ALS/Data/AnimationInstance/Blending/CF_Als_Aim_Out.curve.ron"),
        );
    }
}

fn overlay_target_on(overlay: &OverlayState) -> bool {
    *overlay != OverlayState::Default
}

type OverlayDriverQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static OverlayState,
        Option<&'static mut OverlayBlendState>,
        &'static AnimatedSceneInstance,
    ),
>;

fn overlay_layer_driver(
    mut commands: Commands,
    time: Res<Time>,
    layer_assets: Res<OverlayLayerAssets>,
    curves: Res<Assets<CurveAsset>>,
    mut q: OverlayDriverQuery,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let Some(sample_in) = layer_assets.aim_in.as_ref().and_then(|h| curves.get(h)) else {
        return;
    };
    let Some(sample_out) = layer_assets.aim_out.as_ref().and_then(|h| curves.get(h)) else {
        return;
    };
    let dt = time.delta_secs();

    for (entity, overlay, blend, instance) in &mut q {
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let target_on = overlay_target_on(overlay);
        let weight = match blend {
            Some(mut state) => {
                let t = state.phase_time;
                state.update(target_on, dt, sample_in.sample(t), sample_out.sample(t))
            }
            None => {
                commands.entity(entity).insert(OverlayBlendState::default());
                0.0
            }
        };
        player.set_input_data("overlay_weight", DataValue::from(weight));
    }
}
