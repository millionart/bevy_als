use als_core::character::AlsCharacter;
use als_core::input::OverlayMenuAction;
use als_core::state::OverlayState;
use als_core::transition::AlsStateRequest;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::als_hud::AlsHudState;
use crate::als_hud::COLOR_TEXT;

pub fn menu_display_order(tags: &bevy_gameplay_tag::GameplayTagsManager) -> Vec<OverlayState> {
    let parent = bevy_gameplay_tag::GameplayTag::new("Als.OverlayMode");
    let mut values: Vec<_> = tags.tag_map.keys().filter(|tag| *tag != &parent && tag.matches_tag(&parent, tags))
        .map(OverlayState::from_tag).collect();
    let name_key = |name: &str| {
        let prefix = name.trim_end_matches(|value: char| value.is_ascii_digit());
        match name[prefix.len()..].parse::<i64>() {
            Ok(suffix) => (prefix.to_ascii_lowercase(), suffix),
            Err(_) => (name.to_ascii_lowercase(), -1),
        }
    };
    values.sort_by(|a, b| {
        for (a, b) in a.as_str().split('.').zip(b.as_str().split('.')) {
            let order = name_key(b).cmp(&name_key(a));
            if !order.is_eq() { return order; }
        }
        a.as_str().len().cmp(&b.as_str().len())
    });
    values
}

pub const MENU_DILATION: f32 = 0.35;
pub const OPTION_WIDTH_PX: f32 = 200.0;
pub const OPTION_HEIGHT_PX: f32 = 25.0;
pub const OPTION_GAP_PX: f32 = 4.0;
pub const OPTION_FONT_SIZE_PX: f32 = 14.0;

pub const COLOR_OPTION_SELECTED: Color = Color::linear_rgba(0.15, 0.15, 0.15, 0.5);
pub const COLOR_OPTION_NOT_SELECTED: Color = Color::linear_rgba(0.015, 0.015, 0.015, 0.5);

#[derive(Resource, Default)]
pub struct OverlayMenuState {
    pub open: bool,
    pub selected: Option<OverlayState>,
    pub root: Option<Entity>,
    pub options: Vec<OverlayState>,
}

impl OverlayMenuState {
    pub fn selected_index(&self) -> usize {
        self.options.iter().position(|value| Some(*value) == self.selected).unwrap_or(usize::MAX)
    }
}

#[derive(Component)]
pub struct OverlayMenuOption {
    pub index: usize,
}

pub fn next_index(current: usize, len: usize) -> usize {
    assert!(len > 0);
    if current == 0 || current >= len {
        len - 1
    } else {
        current - 1
    }
}

pub fn previous_index(current: usize, len: usize) -> usize {
    assert!(len > 0);
    if current >= len - 1 { 0 } else { current + 1 }
}

pub fn option_label(state: OverlayState) -> String {
    crate::show_debug_hud::name_to_display_string(&format!("{state:?}"), false)
}

pub struct OverlayModeMenuPlugin;

impl Plugin for OverlayModeMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsHudState>()
            .init_resource::<OverlayMenuState>()
            .add_systems(Update, (menu_control_system, menu_highlight_system).chain());
    }
}

fn menu_control_system(
    mut commands: Commands,
    mut state: ResMut<OverlayMenuState>,
    hud_state: Res<AlsHudState>,
    tags: Res<bevy_gameplay_tag::GameplayTagsManager>,
    mut time: ResMut<Time<Virtual>>,
    menu_held: Query<&Action<OverlayMenuAction>>,
    next_states: Query<&TriggerState, With<Action<als_core::input::NextOverlayModeAction>>>,
    prev_states: Query<&TriggerState, With<Action<als_core::input::PreviousOverlayModeAction>>>,
    characters: Query<(Entity, &OverlayState), With<AlsCharacter>>,
    mut requests: Query<&mut AlsStateRequest>,
    mut prev_held: Local<bool>,
) {
    let held = menu_held.iter().any(|action| **action);
    let pressed = held && !*prev_held;
    let released = !held && *prev_held;
    *prev_held = held;

    let next = next_states
        .iter()
        .any(|state| *state == TriggerState::Fired);
    let prev = prev_states
        .iter()
        .any(|state| *state == TriggerState::Fired);

    if state.open && (next || prev) && !state.options.is_empty() {
        let len = state.options.len();
        let selected_index = if next {
            next_index(state.selected_index(), len)
        } else {
            previous_index(state.selected_index(), len)
        };
        state.selected = Some(state.options[selected_index]);
    }

    if pressed && !state.open {
        state.open = true;
        state.options = menu_display_order(&tags);
        state.selected = characters.iter().next().map(|(_, overlay)| *overlay);
        if !hud_state.slomo_active {
            time.set_relative_speed(MENU_DILATION);
        }
        state.root = Some(spawn_menu(
            &mut commands,
            state.selected_index(),
            &state.options,
        ));
    }

    if released && state.open {
        if let Some(selected) = state.selected
            && let Some((entity, _)) = characters.iter().next() {
            match requests.get_mut(entity) {
                Ok(mut request) => request.overlay_state = Some(selected),
                Err(_) => {
                    commands.entity(entity).insert(AlsStateRequest {
                        overlay_state: Some(selected),
                        ..default()
                    });
                }
            }
        }
        if let Some(root) = state.root.take() {
            commands.entity(root).despawn();
        }
        state.open = false;
        if !hud_state.slomo_active {
            time.set_relative_speed(1.0);
        }
    }
}

fn spawn_menu(
    commands: &mut Commands,
    selected_index: usize,
    display_order: &[OverlayState],
) -> Entity {
    let root = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(
                crate::als_hud::PANEL_RIGHT_MARGIN_PX
                    + crate::als_hud::PANEL_WIDTH_PX
                    + crate::als_hud::SECTION_GAP_PX,
            ),
            top: Val::Px(crate::als_hud::PANEL_TOP_MARGIN_PX),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(OPTION_GAP_PX),
            ..default()
        })
        .id();

    for (index, overlay) in display_order.iter().enumerate() {
        let selected = index == selected_index;
        let option = commands
            .spawn((
                OverlayMenuOption { index },
                Node {
                    width: Val::Px(OPTION_WIDTH_PX),
                    height: Val::Px(OPTION_HEIGHT_PX),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(if selected {
                    COLOR_OPTION_SELECTED
                } else {
                    COLOR_OPTION_NOT_SELECTED
                }),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(option_label(*overlay)),
                    TextFont {
                        font_size: FontSize::Px(OPTION_FONT_SIZE_PX),
                        ..default()
                    },
                    TextColor(COLOR_TEXT),
                ));
            })
            .id();
        commands.entity(root).add_child(option);
    }

    root
}

fn menu_highlight_system(
    state: Res<OverlayMenuState>,
    mut options: Query<(&OverlayMenuOption, &mut BackgroundColor)>,
) {
    if !state.open {
        return;
    }
    for (option, mut background) in &mut options {
        let selected = option.index == state.selected_index();
        let color = if selected {
            COLOR_OPTION_SELECTED
        } else {
            COLOR_OPTION_NOT_SELECTED
        };
        if background.0 != color {
            background.0 = color;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn tag_manager(extra: &[&str]) -> bevy_gameplay_tag::GameplayTagsManager {
        let mut rows: Vec<serde_json::Value> = serde_json::from_str(include_str!("../assets/als_gameplay_tags.json")).unwrap();
        rows.extend(extra.iter().map(|name| serde_json::json!({"tag_name": name, "description": "Test overlay"})));
        let mut world = World::new();
        world.insert_resource(bevy_gameplay_tag::GameplayTagsSettings { json_data: serde_json::to_string(&rows).unwrap(), data_path: None });
        bevy_gameplay_tag::GameplayTagsManager::from_world(&mut world)
    }

    #[test]
    fn menu_uses_registered_descendants_and_source_tree_order() {
        let tags = tag_manager(&["Als.OverlayMode.Custom2", "Als.OverlayMode.Custom10.Detail", "Als.Unrelated.Custom"]);
        let order = menu_display_order(&tags);
        assert_eq!(order.len(), 16);
        assert_eq!(order[0], OverlayState::Torch);
        assert_eq!(*order.last().unwrap(), OverlayState::Barrel);
        let custom = order.iter().position(|tag| tag.as_str() == "Als.OverlayMode.Custom10").unwrap();
        assert_eq!(order[custom + 1].as_str(), "Als.OverlayMode.Custom10.Detail");
        assert_eq!(order[custom + 2].as_str(), "Als.OverlayMode.Custom2");
        let baseline_order = menu_display_order(&tag_manager(&[]));
        assert_eq!(baseline_order.len(), 13);
        assert_eq!(baseline_order[8], OverlayState::Default);
        assert_eq!(next_index(0, order.len()), order.len() - 1);
        assert_eq!(previous_index(order.len() - 1, order.len()), 0);
        assert_eq!(next_index(usize::MAX, order.len()), order.len() - 1);
        assert_eq!(previous_index(usize::MAX, order.len()), 0);
        let empty = bevy_gameplay_tag::GameplayTagsManager { root: Entity::PLACEHOLDER, tag_map: default() };
        assert!(menu_display_order(&empty).is_empty());
    }

    #[test]
    fn menu_commits_the_displayed_custom_tag_and_handles_an_empty_dictionary() {
        let custom = OverlayState::new("Als.OverlayMode.CustomRifle").unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).insert_resource(tag_manager(&[custom.as_str()])).add_plugins(OverlayModeMenuPlugin);
        let character = app.world_mut().spawn((AlsCharacter, custom, AlsStateRequest::default())).id();
        let mut action = Action::<OverlayMenuAction>::new();
        *action = true;
        let action = app.world_mut().spawn(action).id();
        app.update();
        let state = app.world().resource::<OverlayMenuState>();
        assert!(state.open);
        assert_eq!(state.selected, Some(custom));
        assert_eq!(app.world_mut().query::<&OverlayMenuOption>().iter(app.world()).count(), 14);
        app.insert_resource(tag_manager(&[]));
        **app.world_mut().get_mut::<Action<OverlayMenuAction>>(action).unwrap() = false;
        app.update();
        assert_eq!(app.world().get::<AlsStateRequest>(character).unwrap().overlay_state, Some(custom));
        assert!(!app.world().resource::<OverlayMenuState>().open);
        assert_eq!(app.world().resource::<Time<Virtual>>().relative_speed(), 1.0);
        app.world_mut().resource_mut::<bevy_gameplay_tag::GameplayTagsManager>().tag_map.clear();
        **app.world_mut().get_mut::<Action<OverlayMenuAction>>(action).unwrap() = true;
        app.update();
        assert!(app.world().resource::<OverlayMenuState>().options.is_empty());
        *app.world_mut().get_mut::<OverlayState>(character).unwrap() = OverlayState::Default;
        app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().overlay_state = None;
        let next = app.world_mut().spawn((Action::<als_core::input::NextOverlayModeAction>::new(), TriggerState::Fired)).id();
        app.update();
        app.world_mut().despawn(next);
        **app.world_mut().get_mut::<Action<OverlayMenuAction>>(action).unwrap() = false;
        app.update();
        assert!(!app.world().resource::<OverlayMenuState>().open);
        assert_eq!(app.world().get::<AlsStateRequest>(character).unwrap().overlay_state, Some(custom));
    }

    #[test]
    fn option_labels_match_ue_name_display() {
        assert_eq!(option_label(OverlayState::Default), "Default");
        assert_eq!(
            option_label(OverlayState::PistolOneHanded),
            "Pistol One Handed"
        );
        assert_eq!(
            option_label(OverlayState::PistolTwoHanded),
            "Pistol Two Handed"
        );
        assert_eq!(option_label(OverlayState::HandsTied), "Hands Tied");
    }

    #[test]
    fn selection_background_preserves_ue_linear_color_contrast() {
        let selected = COLOR_OPTION_SELECTED.to_linear();
        let unselected = COLOR_OPTION_NOT_SELECTED.to_linear();
        assert!((selected.red - 0.15).abs() < 1e-6);
        assert!((unselected.red - 0.015).abs() < 1e-6);
        assert!((selected.alpha - 0.5).abs() < 1e-6);
        assert!((unselected.alpha - 0.5).abs() < 1e-6);
        assert!(selected.red / unselected.red >= 9.9);
    }
}
