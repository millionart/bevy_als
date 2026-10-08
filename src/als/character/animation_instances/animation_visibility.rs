use bevy::prelude::*;
use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer};
use crate::character_assembly::AlsCharacterAvatar;
use crate::extensions::post_physics_animation::AnimationPoseUpdateGate;

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

impl Plugin for AlsAnimationVisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, refresh_mesh_animation_visibility.before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics))
            .add_observer(|event: On<Discard, AnimationGraphPlayer>, mut commands: Commands| {
                commands.entity(event.entity).try_remove::<AnimationPoseUpdateGate>();
            });
    }
}

fn refresh_mesh_animation_visibility(
    time: Res<Time>,
    spawner: Option<Res<bevy::world_serialization::WorldInstanceSpawner>>,
    meshes: Query<(&ViewVisibility, &InheritedVisibility), With<Mesh3d>>,
    mut avatars: Query<(Entity, &AnimatedSceneInstance, &bevy::world_serialization::WorldInstance, Option<&AlsAnimationTickOption>, Option<&mut AlsMeshRenderTime>), With<AlsCharacterAvatar>>,
    mut players: Query<(&AnimationGraphPlayer, Option<&mut AnimationPoseUpdateGate>)>,
    mut commands: Commands,
) {
    let Some(spawner) = spawner else { return; };
    let now = time.elapsed_secs_f64();
    for (avatar, instance, world_instance, option, render_time) in &mut avatars {
        let rendered = spawner.iter_instance_entities(**world_instance).any(|entity| meshes.get(entity).is_ok_and(|(view, inherited)| view.get() && inherited.get()));
        let previous = render_time.as_ref().map_or(0.0, |time| time.0);
        let last_render = if rendered { (now - time.delta_secs_f64()) as f32 } else { previous };
        if let Some(mut render_time) = render_time { render_time.0 = last_render; }
        else { commands.entity(avatar).insert(AlsMeshRenderTime(last_render)); }
        let recent = (last_render as f64) > now - crate::ab_als_provider::ANIMATION_COMPONENT.recently_rendered_seconds;
        let option = option.copied().unwrap_or(crate::ab_als_provider::ANIMATION_COMPONENT.visibility_tick_option);
        let Ok((player, gate)) = players.get_mut(instance.player_entity()) else { continue; };
        let initialize = player.get_animation_source().is_graph() && player.get_context_arena().is_none_or(|arena| arena.frame_index() == 0);
        let update_graph = initialize || recent || option != AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered;
        let refresh_bones = initialize || recent || option == AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones;
        if let Some(mut gate) = gate {
            gate.update_graph = update_graph;
            gate.refresh_bones = refresh_bones;
            gate.pending_update |= !update_graph;
        } else {
            commands.entity(instance.player_entity()).insert(AnimationPoseUpdateGate { update_graph, refresh_bones, pending_update: true });
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
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer = output.clone();
        let subscriber = bevy::log::tracing_subscriber::fmt()
            .with_ansi(false).without_time()
            .with_writer(move || Capture(writer.clone())).finish();
        bevy::log::tracing::subscriber::with_default(subscriber, || {
            let mut app = App::new();
            app.add_plugins((bevy::log::LogPlugin::default(), AlsAnimationVisibilityPlugin));
            bevy::ecs::error::warn("discard logging probe".into(),
                bevy::ecs::error::ErrorContext::Command { name: "discard logging probe".into() });
            assert!(String::from_utf8(output.lock().unwrap().clone()).unwrap().contains("discard logging probe"));
            output.lock().unwrap().clear();
            let gate = AnimationPoseUpdateGate { update_graph: false, refresh_bones: false, pending_update: false };
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(default()), gate)).id();
            app.world_mut().entity_mut(player).remove::<AnimationGraphPlayer>();
            app.world_mut().flush();
            assert!(app.world().get::<AnimationPoseUpdateGate>(player).is_none());
            app.world_mut().entity_mut(player).insert((AnimationGraphPlayer::new(default()), gate));
            app.world_mut().entity_mut(player).insert(AnimationGraphPlayer::new(default()));
            app.world_mut().flush();
            assert!(app.world().get::<AnimationPoseUpdateGate>(player).is_none());
            app.world_mut().entity_mut(player).insert(gate);
            app.world_mut().commands().entity(player).despawn();
            app.world_mut().flush();
            assert!(app.world().get_entity(player).is_err());
        });
        let output = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("Encountered an error"), "Discard cleanup queued a command against a destroyed player: {output}");
    }

    #[test]
    fn visibility_policy_uses_any_render_view_strict_recentness_and_pending_resume() {
        let (mut app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let mut pending = vec![avatar];
        let mut meshes = Vec::new();
        while let Some(entity) = pending.pop() {
            if app.world().get::<Mesh3d>(entity).is_some() { meshes.push(entity); }
            if let Some(children) = app.world().get::<Children>(entity) { pending.extend(children.iter()); }
        }
        assert!(!meshes.is_empty());
        app.world_mut().spawn((Mesh3d(Handle::default()), ViewVisibility::VISIBLE, InheritedVisibility::VISIBLE, ChildOf(avatar)));
        for entity in &meshes { app.world_mut().entity_mut(*entity).insert((ViewVisibility::HIDDEN, InheritedVisibility::VISIBLE)); }
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        for option in [AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered, AlsAnimationTickOption::AlwaysTickPose, AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones] {
            for age in [0.99_f64, 1.0, 1.01] {
                app.world_mut().entity_mut(avatar).insert((option, AlsMeshRenderTime((now - age) as f32)));
                app.world_mut().run_system_once(refresh_mesh_animation_visibility).unwrap();
                let gate = app.world().get::<AnimationPoseUpdateGate>(player).unwrap();
                assert_eq!(gate.update_graph, ((now - age) as f32 as f64) > now - 1.0 || option != AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered);
                assert_eq!(gate.refresh_bones, ((now - age) as f32 as f64) > now - 1.0 || option == AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
            }
        }
        app.world_mut().entity_mut(avatar).insert((AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered, AlsMeshRenderTime((now - 2.0) as f32)));
        app.world_mut().run_system_once(refresh_mesh_animation_visibility).unwrap();
        assert!(app.world().get::<AnimationPoseUpdateGate>(player).unwrap().pending_update);
        for entity in meshes.iter().take(1) { app.world_mut().entity_mut(*entity).insert(ViewVisibility::VISIBLE); }
        app.update();
        let gate = app.world().get::<AnimationPoseUpdateGate>(player).unwrap();
        assert!(gate.update_graph && gate.refresh_bones && !gate.pending_update);
        assert!(app.world().get::<AnimationGraphPlayer>(player).unwrap().get_global_input_data("animation.pending_update").unwrap().as_bool().unwrap());
        app.update();
        assert!(!app.world().get::<AnimationGraphPlayer>(player).unwrap().get_global_input_data("animation.pending_update").unwrap().as_bool().unwrap());
        for entity in &meshes { app.world_mut().entity_mut(*entity).insert(ViewVisibility::HIDDEN); }
        let before = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_context_arena().unwrap().frame_index();
        for _ in 0..4 { app.update(); }
        assert!(app.world().get::<AnimationGraphPlayer>(player).unwrap().get_context_arena().unwrap().frame_index() > before);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
        for _ in 0..13 { app.update(); }
        assert!(!app.world().get::<AnimationPoseUpdateGate>(player).unwrap().update_graph);
        assert!(app.world().get::<AnimationPoseUpdateGate>(player).unwrap().pending_update);
    }
}
