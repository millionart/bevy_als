use als_core::character::AlsCharacter;
use als_core::input::OverlayMenuAction;
use als_core::state::OverlayState;
use als_core::transition::AlsStateRequest;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::als_hud::AlsHudState;
use crate::als_hud::COLOR_TEXT;

pub const MENU_ORDER: [OverlayState; 13] = [
    OverlayState::Torch,
    OverlayState::Rifle,
    OverlayState::PistolTwoHanded,
    OverlayState::PistolOneHanded,
    OverlayState::Masculine,
    OverlayState::Injured,
    OverlayState::HandsTied,
    OverlayState::Feminine,
    OverlayState::Default,
    OverlayState::Box,
    OverlayState::Bow,
    OverlayState::Binoculars,
    OverlayState::Barrel,
];

pub const MENU_DILATION: f32 = 0.35;
pub const OPTION_WIDTH_PX: f32 = 200.0;
pub const OPTION_HEIGHT_PX: f32 = 25.0;
pub const OPTION_GAP_PX: f32 = 4.0;
pub const OPTION_FONT_SIZE_PX: f32 = 14.0;

pub const COLOR_OPTION_SELECTED: Color = Color::srgba(0.15, 0.15, 0.15, 0.5);
pub const COLOR_OPTION_NOT_SELECTED: Color = Color::srgba(0.015, 0.015, 0.015, 0.5);

#[derive(Resource, Default)]
pub struct OverlayMenuState {
    pub open: bool,
    pub selected_index: usize,
    pub root: Option<Entity>,
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
    mut time: ResMut<Time<Virtual>>,
    menu_held: Query<&Action<OverlayMenuAction>>,
    next_events: Query<&ActionEvents, With<Action<als_core::input::NextOverlayModeAction>>>,
    prev_events: Query<&ActionEvents, With<Action<als_core::input::PreviousOverlayModeAction>>>,
    characters: Query<(Entity, &OverlayState), With<AlsCharacter>>,
    mut requests: Query<&mut AlsStateRequest>,
    mut prev_held: Local<bool>,
) {
    let held = menu_held.iter().any(|action| **action);
    let pressed = held && !*prev_held;
    let released = !held && *prev_held;
    *prev_held = held;

    let next = next_events
        .iter()
        .any(|events| events.contains(ActionEvents::START));
    let prev = prev_events
        .iter()
        .any(|events| events.contains(ActionEvents::START));

    if state.open && (next || prev) {
        let len = MENU_ORDER.len();
        state.selected_index = if next {
            next_index(state.selected_index, len)
        } else {
            previous_index(state.selected_index, len)
        };
    }

    if pressed && !state.open {
        state.open = true;
        state.selected_index = characters
            .iter()
            .next()
            .map(|(_, overlay)| {
                MENU_ORDER
                    .iter()
                    .position(|candidate| candidate == overlay)
                    .unwrap_or(8)
            })
            .unwrap_or(8);
        if !hud_state.slomo_active {
            time.set_relative_speed(MENU_DILATION);
        }
        state.root = Some(spawn_menu(&mut commands, state.selected_index));
    }

    if released && state.open {
        let selected = MENU_ORDER[state.selected_index];
        if let Some((entity, _)) = characters.iter().next() {
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

fn spawn_menu(commands: &mut Commands, selected_index: usize) -> Entity {
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

    for (index, overlay) in MENU_ORDER.iter().enumerate() {
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
        let selected = option.index == state.selected_index;
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
mod tests {
    use super::*;

    #[test]
    fn menu_order_covers_every_overlay_exactly_once() {
        let all = [
            OverlayState::Default,
            OverlayState::Masculine,
            OverlayState::Feminine,
            OverlayState::Injured,
            OverlayState::HandsTied,
            OverlayState::Rifle,
            OverlayState::PistolOneHanded,
            OverlayState::PistolTwoHanded,
            OverlayState::Bow,
            OverlayState::Torch,
            OverlayState::Binoculars,
            OverlayState::Box,
            OverlayState::Barrel,
        ];
        assert_eq!(MENU_ORDER.len(), all.len());
        for state in all {
            let count = MENU_ORDER
                .iter()
                .filter(|candidate| **candidate == state)
                .count();
            assert_eq!(count, 1, "{state:?} appears {count} times");
        }
    }

    #[test]
    fn default_is_selected_row_in_reference_screenshot() {
        assert_eq!(MENU_ORDER[8], OverlayState::Default);
        assert_eq!(MENU_ORDER[0], OverlayState::Torch);
        assert_eq!(MENU_ORDER[12], OverlayState::Barrel);
    }

    #[test]
    fn wrap_semantics_match_ue_select_functions() {
        let len = MENU_ORDER.len();
        assert_eq!(next_index(0, len), len - 1, "next from first wraps to last");
        assert_eq!(next_index(8, len), 7);
        assert_eq!(
            previous_index(len - 1, len),
            0,
            "previous from last wraps to first"
        );
        assert_eq!(previous_index(8, len), 9);
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
}
