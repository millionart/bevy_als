use als_core::character::AlsCharacter;
use bevy::prelude::*;

use crate::character_assembly::AlsPlayerCharacter;
use crate::show_debug_hud::{AlsShowDebugHudSet, AlsShowDebugHudState};

#[derive(Component)]
pub struct AlsDebugTarget;

#[derive(Component)]
pub struct AlsLastDebugRender(pub f64);

#[derive(Resource, Default)]
struct DebugTargetSelection {
    cached: Option<Entity>,
    index: i32,
    visible: bool,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AlsDebugTargetSet;

pub struct AlsDebugTargetPlugin;

impl Plugin for AlsDebugTargetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugTargetSelection>()
            .add_systems(
                Update,
                select_target
                    .in_set(AlsDebugTargetSet)
                    .after(AlsShowDebugHudSet::Input),
            )
            .add_systems(
                PostUpdate,
                record_rendered_characters
                    .after(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            );
    }
}

fn select_target(
    mut commands: Commands,
    mut selection: ResMut<DebugTargetSelection>,
    state: Option<Res<AlsShowDebugHudState>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    time: Res<Time>,
    players: Query<Entity, (With<AlsCharacter>, With<AlsPlayerCharacter>)>,
    actors: Query<(Entity, Option<&AlsLastDebugRender>), With<AlsCharacter>>,
    marked: Query<Entity, With<AlsDebugTarget>>,
) {
    let visible = state.as_ref().is_some_and(|state| state.visible);
    if visible != selection.visible {
        selection.cached = None;
        selection.visible = visible;
    }
    if selection
        .cached
        .is_some_and(|entity| !actors.contains(entity))
    {
        selection.cached = None;
    }
    let player = players.single().ok();
    if visible && selection.cached.is_none() {
        selection.cached = player;
    }
    let next = keys
        .as_ref()
        .is_some_and(|keys| keys.just_pressed(KeyCode::PageUp));
    let previous = keys
        .as_ref()
        .is_some_and(|keys| keys.just_pressed(KeyCode::PageDown));
    if next || previous {
        let threshold = 0.2f64.max(time.delta_secs_f64() + 0.0001);
        let mut targets: Vec<_> = actors
            .iter()
            .filter(|(_, stamp)| {
                stamp.is_some_and(|stamp| time.elapsed_secs_f64() - stamp.0 <= threshold)
            })
            .map(|(entity, _)| entity)
            .collect();
        targets.sort_by_key(|entity| entity.to_bits());
        for forward in [next.then_some(true), previous.then_some(false)]
            .into_iter()
            .flatten()
        {
            if let Some(index) = targets
                .iter()
                .position(|entity| Some(*entity) == selection.cached)
            {
                selection.index = index as i32;
            }
            if targets.is_empty() {
                selection.index = -1;
                selection.cached = player;
                continue;
            }
            selection.index = if forward {
                (selection.index + 1) % targets.len() as i32
            } else if selection.index <= 0 {
                targets.len() as i32 - 1
            } else {
                selection.index - 1
            };
            selection.cached = usize::try_from(selection.index).ok().and_then(|index| targets.get(index)).copied().or(player);
        }
    }
    let target = selection.cached.or(player);
    for entity in &marked {
        if Some(entity) != target {
            commands.entity(entity).remove::<AlsDebugTarget>();
        }
    }
    if let Some(target) = target
        && !marked.contains(target)
    {
        commands.entity(target).insert(AlsDebugTarget);
    }
}

fn record_rendered_characters(
    mut commands: Commands,
    time: Res<Time>,
    meshes: Query<(Entity, &ViewVisibility), With<bevy::mesh::skinning::SkinnedMesh>>,
    parents: Query<&ChildOf>,
    avatars: Query<&crate::character_assembly::AlsCharacterAvatar>,
    overlay_objects: Query<(), With<crate::overlay_object::AlsOverlayObject>>,
) {
    for (entity, visibility) in &meshes {
        if !visibility.get() {
            continue;
        }
        for ancestor in std::iter::once(entity).chain(parents.iter_ancestors(entity)) {
            if overlay_objects.contains(ancestor) {
                break;
            }
            if let Ok(owner) = avatars.get(ancestor) {
                commands
                    .entity(owner.character)
                    .try_insert(AlsLastDebugRender(time.elapsed_secs_f64()));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_target_list_then_new_candidates_starts_at_first() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsDebugTargetPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugHudState { visible: true, ..default() });
        app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::PageUp);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();
        let a = app.world_mut().spawn((AlsCharacter, AlsLastDebugRender(0.0))).id();
        let b = app.world_mut().spawn((AlsCharacter, AlsLastDebugRender(0.0))).id();
        let first = [a, b].into_iter().min_by_key(|entity| entity.to_bits()).unwrap();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::PageUp);
        app.update();
        assert!(app.world().get::<AlsDebugTarget>(first).is_some());
    }

    #[test]
    fn target_cycles_recent_pawns_and_resets_when_master_changes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsDebugTargetPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugHudState {
                visible: true,
                ..default()
            });
        let player = app
            .world_mut()
            .spawn((AlsCharacter, AlsPlayerCharacter, AlsLastDebugRender(0.0)))
            .id();
        let other = app
            .world_mut()
            .spawn((AlsCharacter, AlsLastDebugRender(0.0)))
            .id();
        app.world_mut()
            .spawn((AlsCharacter, AlsLastDebugRender(-1.0)));
        app.update();
        assert!(app.world().get::<AlsDebugTarget>(player).is_some());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::PageUp);
        app.update();
        assert!(app.world().get::<AlsDebugTarget>(other).is_some());
        assert!(app.world().get::<AlsDebugTarget>(player).is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut().despawn(other);
        app.update();
        assert!(app.world().get::<AlsDebugTarget>(player).is_some());
        app.world_mut()
            .resource_mut::<AlsShowDebugHudState>()
            .visible = false;
        app.update();
        assert!(app.world().get::<AlsDebugTarget>(player).is_some());
    }

    #[test]
    fn owning_player_legend_does_not_claim_another_targets_categories() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::als_hud::AlsHudPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugHudState {
                visible: true,
                state: true,
                ..default()
            });
        app.world_mut()
            .spawn((AlsCharacter, AlsPlayerCharacter, AlsLastDebugRender(0.0)));
        app.world_mut()
            .spawn((AlsCharacter, AlsLastDebugRender(0.0)));
        app.update();
        let state_color = |world: &mut World| {
            world
                .query::<(&crate::als_hud::AlsHudColorTarget, &TextColor)>()
                .iter(world)
                .find(|(target, _)| {
                    **target
                        == crate::als_hud::AlsHudColorTarget::Category(
                            crate::show_debug_hud::DebugCategory::State,
                        )
                })
                .unwrap()
                .1
                .0
        };
        assert_eq!(state_color(app.world_mut()), crate::als_hud::COLOR_ENABLED);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::PageUp);
        app.update();
        assert_eq!(state_color(app.world_mut()), crate::als_hud::COLOR_DISABLED);
    }

    #[test]
    fn render_history_uses_visible_avatar_meshes_and_hitch_tolerance() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsDebugTargetPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(0.5)))
            .insert_resource(AlsShowDebugHudState { visible: true, ..default() });
        let player = app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter, AlsLastDebugRender(0.0))).id();
        let other = app.world_mut().spawn(AlsCharacter).id();
        let avatar = app.world_mut().spawn(crate::character_assembly::AlsCharacterAvatar { character: other }).id();
        app.world_mut().spawn((bevy::mesh::skinning::SkinnedMesh { inverse_bindposes: default(), joints: Vec::new() },
            ViewVisibility::VISIBLE, ChildOf(avatar)));
        app.update();
        assert!(app.world().get::<AlsLastDebugRender>(other).is_some());
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::PageUp);
        app.update();
        assert!(app.world().get::<AlsDebugTarget>(other).is_some());
        assert!(app.world().get::<AlsDebugTarget>(player).is_none());
    }
}
