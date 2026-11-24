use crate::als::roll::AlsRollState;
use crate::character_assembly::AlsCharacterAvatar;
use crate::extensions::post_physics_animation::AnimationPoseUpdateGate;
use crate::get_up::AlsGetUpState;
use crate::mantle_drive::AlsMantleMontages;
use crate::roll_drive::AlsRollTail;
use crate::stop_link::AlsTransitionMontageStack;
use crate::turn_layer::AlsTurnMontageStack;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub enum AlsAnimationTickOption {
    #[default]
    AlwaysTickPoseAndRefreshBones,
    AlwaysTickPose,
    OnlyTickMontagesWhenNotRendered,
}

#[derive(Component, Default)]
struct AlsMeshRenderTime(f32);

pub struct AlsAnimationVisibilityPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AlsAnimationVisibilitySet;

impl Plugin for AlsAnimationVisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsAnimationVisibilitySet
                .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            refresh_mesh_animation_visibility.in_set(AlsAnimationVisibilitySet),
        )
        .add_observer(
            |event: On<Discard<AnimationGraphPlayer>>, mut commands: Commands| {
                commands
                    .entity(event.entity)
                    .try_remove::<AnimationPoseUpdateGate>();
            },
        );
    }
}

fn refresh_mesh_animation_visibility(
    time: Res<Time>,
    virtual_time: Res<Time<bevy::time::Virtual>>,
    spawner: Option<Res<bevy::world_serialization::WorldInstanceSpawner>>,
    meshes: Query<(&ViewVisibility, &InheritedVisibility), With<Mesh3d>>,
    mut avatars: Query<(
        Entity,
        &AlsCharacterAvatar,
        &AnimatedSceneInstance,
        &bevy::world_serialization::WorldInstance,
        Option<&AlsAnimationTickOption>,
        Option<&mut AlsMeshRenderTime>,
        Option<&AlsTurnMontageStack>,
    )>,
    actor_montages: Query<(
        Option<&AlsTransitionMontageStack>,
        Option<&AlsRollState>,
        Option<&AlsGetUpState>,
        Option<&AlsMantleMontages>,
        Option<&Children>,
    )>,
    roll_tails: Query<(), With<AlsRollTail>>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut players: Query<(
        &mut AnimationGraphPlayer,
        Option<&mut AnimationPoseUpdateGate>,
        Option<&mut crate::AlsAnimationRate>,
    )>,
    mut commands: Commands,
) {
    let Some(spawner) = spawner else {
        return;
    };
    let now = time.elapsed_secs_f64();
    for (avatar, owner, instance, world_instance, option, render_time, turns) in &mut avatars {
        let rendered = spawner
            .iter_instance_entities(**world_instance)
            .any(|entity| {
                meshes
                    .get(entity)
                    .is_ok_and(|(view, inherited)| view.get() && inherited.get())
            });
        let previous = render_time.as_ref().map_or(0.0, |time| time.0);
        let last_render = if rendered {
            (now - time.delta_secs_f64()) as f32
        } else {
            previous
        };
        if let Some(mut render_time) = render_time {
            render_time.0 = last_render;
        } else {
            commands
                .entity(avatar)
                .insert(AlsMeshRenderTime(last_render));
        }
        let recent = (last_render as f64)
            > now - crate::ab_als_provider::ANIMATION_COMPONENT.recently_rendered_seconds;
        let option = option
            .copied()
            .unwrap_or(crate::ab_als_provider::ANIMATION_COMPONENT.visibility_tick_option);
        let Ok((mut player, gate, rate)) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        if let Some(mut rate) = rate {
            rate.visible = rendered;
        }
        let initialize = player.get_animation_source().is_graph()
            && player
                .get_context_arena()
                .is_none_or(|arena| arena.frame_index() == 0);
        let mut update_graph =
            recent || option != AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered;
        let refresh_bones =
            initialize || recent || option == AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones;
        if !update_graph {
            let montage_present = turns.is_some_and(AlsTurnMontageStack::has_montage_at_pose_tick)
                || actor_montages.get(owner.character).is_ok_and(
                    |(transitions, roll, get_up, mantle, children)| {
                        transitions.is_some_and(AlsTransitionMontageStack::has_montage_at_pose_tick)
                            || roll.is_some_and(|roll| roll.active)
                            || get_up.is_some()
                            || mantle.is_some_and(|mantle| !mantle.entries.is_empty())
                            || children.is_some_and(|children| {
                                children.iter().any(|child| roll_tails.contains(child))
                            })
                    },
                );
            if montage_present {
                let delta = time.delta_secs()
                    * custom_dilations
                        .get(owner.character)
                        .map_or(1.0, |value| value.0);
                update_graph = delta == 0.0 && !virtual_time.is_paused();
                if !update_graph && !initialize {
                    crate::extensions::source_instance::reset_source_call_relevancy(&mut player);
                }
            }
        }
        if let Some(mut gate) = gate {
            gate.update_graph = update_graph;
            gate.refresh_bones = refresh_bones;
            gate.pending_update |= !update_graph;
        } else {
            commands
                .entity(instance.player_entity())
                .insert(AnimationPoseUpdateGate {
                    update_graph,
                    refresh_bones,
                    pending_update: true,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn player_discard_cleans_live_gates_and_allows_entity_destruction() {
        use std::sync::{Arc, Mutex};
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer = output.clone();
        let subscriber = bevy::log::tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_writer(move || Capture(writer.clone()))
            .finish();
        bevy::log::tracing::subscriber::with_default(subscriber, || {
            let mut app = App::new();
            app.add_plugins((
                bevy::log::LogPlugin::default(),
                AlsAnimationVisibilityPlugin,
            ));
            bevy::ecs::error::warn(
                "discard logging probe".into(),
                bevy::ecs::error::ErrorContext::Command {
                    name: "discard logging probe".into(),
                },
            );
            assert!(
                String::from_utf8(output.lock().unwrap().clone())
                    .unwrap()
                    .contains("discard logging probe")
            );
            output.lock().unwrap().clear();
            let gate = AnimationPoseUpdateGate {
                update_graph: false,
                refresh_bones: false,
                pending_update: false,
            };
            let player = app
                .world_mut()
                .spawn((AnimationGraphPlayer::new(default()), gate))
                .id();
            app.world_mut()
                .entity_mut(player)
                .remove::<AnimationGraphPlayer>();
            app.world_mut().flush();
            assert!(app.world().get::<AnimationPoseUpdateGate>(player).is_none());
            app.world_mut()
                .entity_mut(player)
                .insert((AnimationGraphPlayer::new(default()), gate));
            app.world_mut()
                .entity_mut(player)
                .insert(AnimationGraphPlayer::new(default()));
            app.world_mut().flush();
            assert!(app.world().get::<AnimationPoseUpdateGate>(player).is_none());
            app.world_mut().entity_mut(player).insert(gate);
            app.world_mut().commands().entity(player).despawn();
            app.world_mut().flush();
            assert!(app.world().get_entity(player).is_err());
        });
        let output = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains("Encountered an error"),
            "Discard cleanup queued a command against a destroyed player: {output}"
        );
    }

    #[test]
    fn visibility_policy_uses_any_render_view_strict_recentness_and_pending_resume() {
        let (mut app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let mut pending = vec![avatar];
        let mut meshes = Vec::new();
        while let Some(entity) = pending.pop() {
            if app.world().get::<Mesh3d>(entity).is_some() {
                meshes.push(entity);
            }
            if let Some(children) = app.world().get::<Children>(entity) {
                pending.extend(children.iter());
            }
        }
        assert!(!meshes.is_empty());
        app.world_mut().spawn((
            Mesh3d(Handle::default()),
            ViewVisibility::VISIBLE,
            InheritedVisibility::VISIBLE,
            ChildOf(avatar),
        ));
        for entity in &meshes {
            app.world_mut()
                .entity_mut(*entity)
                .insert((ViewVisibility::HIDDEN, InheritedVisibility::VISIBLE));
        }
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        for option in [
            AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered,
            AlsAnimationTickOption::AlwaysTickPose,
            AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones,
        ] {
            for age in [0.99_f64, 1.0, 1.01] {
                app.world_mut()
                    .entity_mut(avatar)
                    .insert((option, AlsMeshRenderTime((now - age) as f32)));
                app.world_mut()
                    .run_system_once(refresh_mesh_animation_visibility)
                    .unwrap();
                let gate = app.world().get::<AnimationPoseUpdateGate>(player).unwrap();
                assert_eq!(
                    gate.update_graph,
                    ((now - age) as f32 as f64) > now - 1.0
                        || option != AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered
                );
                assert_eq!(
                    gate.refresh_bones,
                    ((now - age) as f32 as f64) > now - 1.0
                        || option == AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones
                );
            }
        }
        app.world_mut().entity_mut(avatar).insert((
            AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered,
            AlsMeshRenderTime((now - 2.0) as f32),
        ));
        app.world_mut()
            .run_system_once(refresh_mesh_animation_visibility)
            .unwrap();
        assert!(
            app.world()
                .get::<AnimationPoseUpdateGate>(player)
                .unwrap()
                .pending_update
        );
        for entity in meshes.iter().take(1) {
            app.world_mut()
                .entity_mut(*entity)
                .insert(ViewVisibility::VISIBLE);
        }
        app.update();
        let gate = app.world().get::<AnimationPoseUpdateGate>(player).unwrap();
        assert!(gate.update_graph && gate.refresh_bones && !gate.pending_update);
        assert!(
            app.world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_global_input_data("animation.pending_update")
                .unwrap()
                .as_bool()
                .unwrap()
        );
        app.update();
        assert!(
            !app.world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_global_input_data("animation.pending_update")
                .unwrap()
                .as_bool()
                .unwrap()
        );
        for entity in &meshes {
            app.world_mut()
                .entity_mut(*entity)
                .insert(ViewVisibility::HIDDEN);
        }
        let before = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_context_arena()
            .unwrap()
            .frame_index();
        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index()
                > before
        );
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(0.1),
        ));
        for _ in 0..13 {
            app.update();
        }
        assert!(
            !app.world()
                .get::<AnimationPoseUpdateGate>(player)
                .unwrap()
                .update_graph
        );
        assert!(
            app.world()
                .get::<AnimationPoseUpdateGate>(player)
                .unwrap()
                .pending_update
        );
    }

    #[test]
    fn hidden_roll_updates_the_graph_at_zero_component_delta() {
        use crate::{als::input::AlsInputCommand, als_camera::AlsCameraCustomTimeDilation};
        use bevy::time::{TimeUpdateStrategy, Virtual};
        use std::time::Duration;

        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        app.world_mut()
            .entity_mut(avatar)
            .remove::<AlsAnimationTickOption>()
            .insert(Visibility::Hidden);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        for _ in 0..16 {
            app.update();
        }
        let frame = |app: &App| {
            app.world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index()
        };
        let idle_frame = frame(&app);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        assert!(
            !app.world()
                .get::<AnimationPoseUpdateGate>(player)
                .unwrap()
                .update_graph
        );
        assert_eq!(frame(&app), idle_frame);

        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        app.world_mut()
            .get_mut::<AlsInputCommand>(character)
            .unwrap()
            .roll_pressed = true;
        app.update();
        app.world_mut()
            .get_mut::<AlsInputCommand>(character)
            .unwrap()
            .roll_pressed = false;
        assert!(app.world().get::<AlsRollState>(character).unwrap().active);
        assert_eq!(frame(&app), idle_frame);

        for (delta, dilation, updates_graph) in [
            (0.0, 1.0, true),
            (0.5e-6, 1.0, false),
            (1e-6, 1.0, false),
            (1.0 / 60.0, 1.0, false),
            (1.0 / 60.0, 0.0, true),
        ] {
            let before = frame(&app);
            let position = app
                .world()
                .get::<AlsRollState>(character)
                .unwrap()
                .clip_time();
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                delta,
            )));
            app.world_mut()
                .entity_mut(character)
                .insert(AlsCameraCustomTimeDilation(dilation));
            app.update();
            let gate = app.world().get::<AnimationPoseUpdateGate>(player).unwrap();
            assert_eq!(
                gate.update_graph, updates_graph,
                "delta={delta} dilation={dilation}"
            );
            assert!(!gate.refresh_bones);
            assert_eq!(frame(&app), before + u64::from(updates_graph));
            if updates_graph {
                assert_eq!(
                    app.world()
                        .get::<AlsRollState>(character)
                        .unwrap()
                        .clip_time(),
                    position
                );
            }
        }
        let before_pause = frame(&app);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.update();
        assert_eq!(frame(&app), before_pause);
    }

    #[test]
    fn hidden_montage_ticks_restart_the_actual_head_relevance() {
        use crate::als::aim::AlsLastFreeRotationMode;
        use crate::als::state::RotationMode;
        use crate::als::view::AlsViewPitch;
        use crate::stop_link::{AlsTransitionPlayback, play_transition};

        for (play_montage, terminal) in [(false, false), (true, false), (true, true)] {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(character).insert((
                RotationMode::ViewDirection,
                AlsLastFreeRotationMode(RotationMode::ViewDirection),
                AlsViewPitch(0.0),
            ));
            app.world_mut()
                .entity_mut(avatar)
                .insert(AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
            for _ in 0..3 {
                app.update();
            }
            let player = app
                .world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity();
            app.world_mut()
                .entity_mut(avatar)
                .insert(Visibility::Hidden);
            app.update();
            let frame = app
                .world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index();
            let now = app.world().resource::<Time>().elapsed_secs_f64();
            app.world_mut().entity_mut(avatar).insert((
                AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered,
                AlsMeshRenderTime((now - 2.0) as f32),
            ));
            app.world_mut()
                .entity_mut(character)
                .insert(AlsViewPitch(std::f32::consts::FRAC_PI_4));
            if play_montage {
                let mut montage = AlsTransitionPlayback::standing_dynamic(0.0);
                if terminal {
                    montage.advance(montage.blend_in);
                    montage.request_stop();
                    montage.advance(montage.blend_out - 0.008);
                    assert!(montage.active);
                }
                app.world_mut()
                    .run_system_once(move |mut commands: Commands| {
                        play_transition(&mut commands, character, montage);
                    })
                    .unwrap();
            }
            let hidden_updates = if terminal { 1 } else { 6 };
            for _ in 0..hidden_updates {
                app.update();
            }
            let gate = app.world().get::<AnimationPoseUpdateGate>(player).unwrap();
            assert!(!gate.update_graph && gate.pending_update);
            let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert_eq!(graph.get_context_arena().unwrap().frame_index(), frame);
            if play_montage {
                let montage = app
                    .world()
                    .get::<AlsTransitionMontageStack>(character)
                    .unwrap()
                    .latest()
                    .unwrap();
                assert!(montage.clip_time > 0.0);
                assert_eq!(montage.active, !terminal);
            }
            let now = app.world().resource::<Time>().elapsed_secs_f64();
            app.world_mut()
                .entity_mut(avatar)
                .insert((Visibility::Visible, AlsMeshRenderTime(now as f32)));
            app.update();
            let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert!(graph.get_context_arena().unwrap().frame_index() > frame);
            let target = graph
                .get_global_input_data("view_state.pitch_angle")
                .unwrap()
                .as_f32()
                .unwrap();
            assert!((target - 45.0).abs() < 1e-4);
            let pitch = crate::head_layer::head_state_from_player(graph).pitch_angle;
            if play_montage {
                assert!(
                    (pitch - target).abs() < 1e-4,
                    "Hidden montage (terminal={terminal}) resumed at {pitch}"
                );
            } else {
                assert!(
                    (pitch - target).abs() > 1.0,
                    "Idle hiding restarted the head at {pitch}"
                );
            }
        }
    }

    #[test]
    fn head_resumes_with_real_time_after_hidden_idle() {
        use crate::als::input_request::AlsFirstPersonView;
        use crate::als::view::AlsViewPitch;
        use bevy::time::Virtual;

        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut()
            .entity_mut(character)
            .insert((AlsFirstPersonView, AlsViewPitch(0.0)));
        app.world_mut().entity_mut(avatar).insert((
            Visibility::Hidden,
            AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones,
        ));
        for _ in 0..3 {
            app.update();
        }
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let frame = graph.get_context_arena().unwrap().frame_index();
        let head = crate::head_layer::head_state_from_player(graph);
        assert!(!head.initialization_required && head.pitch_angle.abs() < 1e-4);
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        app.world_mut().entity_mut(avatar).insert((
            AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered,
            AlsMeshRenderTime((now - 2.0) as f32),
        ));
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed(0.25);
        for _ in 0..4 {
            app.update();
        }

        let scalar = |player: &AnimationGraphPlayer, key| {
            player.get_global_input_data(key).unwrap().as_f32().unwrap()
        };
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert_eq!(graph.get_context_arena().unwrap().frame_index(), frame);
        app.world_mut()
            .entity_mut(character)
            .insert(AlsViewPitch(std::f32::consts::FRAC_PI_4));
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        app.world_mut()
            .entity_mut(avatar)
            .insert((Visibility::Visible, AlsMeshRenderTime(now as f32)));
        app.update();

        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let pitch = crate::head_layer::head_state_from_player(graph).pitch_angle;
        assert_eq!(graph.get_context_arena().unwrap().frame_index(), frame + 1);
        let real_delta = app.world().resource::<Time<bevy::time::Real>>().delta_secs();
        assert!(
            (scalar(graph, "animation.delta_seconds_without_time_dilation") - real_delta).abs()
                < 1e-6
        );
        let expected = crate::als::head::damper_exact_angle(
            head.pitch_angle, 45.0, real_delta, 0.01,
        );
        assert!(
            (pitch - expected).abs() < 1e-3,
            "First-person head resumed at {pitch}"
        );
        app.update();
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert!(
            (scalar(graph, "animation.delta_seconds_without_time_dilation") - 1.0 / 60.0).abs()
                < 1e-6
        );
    }
}
