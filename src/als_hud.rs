use crate::show_debug_hud::{AlsShowDebugHudState, DebugCategory};
use als_core::input::SlomoAction;
use als_core::input::ToggleUiAction;
use bevy::prelude::*;
use bevy::ui::Display;
use bevy_enhanced_input::prelude::*;

pub const PANEL_WIDTH_PX: f32 = 340.0;
pub const PANEL_RIGHT_MARGIN_PX: f32 = 25.0;
pub const PANEL_TOP_MARGIN_PX: f32 = 25.0;
pub const ROW_GAP_PX: f32 = 4.0;
pub const SECTION_GAP_PX: f32 = 8.0;
pub const LABEL_GROW: f32 = 1.0;
pub const KEY_CELL_GROW: f32 = 0.7;
pub const DEBUG_KEY_CELL_GROW: f32 = 0.6;
pub const CELL_GAP_PX: f32 = 4.0;
pub const FONT_SIZE_DEFAULT_PX: f32 = 12.0;
pub const FONT_SIZE_VALUE_PX: f32 = 10.0;
pub const SLMO_TIME_SCALE: f32 = 0.15;

pub const COLOR_ENABLED: Color = Color::linear_rgba(0.0, 1.0, 0.0, 1.0);
pub const COLOR_DISABLED: Color = Color::linear_rgba(0.5, 0.5, 0.5, 1.0);
pub const COLOR_TITLE_ACCENT: Color = Color::linear_rgba(1.0, 0.666_667, 0.0, 1.0);
pub const COLOR_TEXT: Color = Color::linear_rgba(1.0, 1.0, 1.0, 1.0);
pub const COLOR_BORDER_WEAK: Color = Color::linear_rgba(0.015, 0.015, 0.015, 0.25);
pub const COLOR_BORDER_STRONG: Color = Color::linear_rgba(0.015, 0.015, 0.015, 0.5);

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsHudState {
    pub ui_visible: bool,
    pub slomo_active: bool,
}

impl Default for AlsHudState {
    fn default() -> Self {
        Self {
            ui_visible: true,
            slomo_active: false,
        }
    }
}

impl AlsHudState {
    pub fn toggle_color(active: bool) -> Color {
        if active {
            COLOR_ENABLED
        } else {
            COLOR_DISABLED
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsHudColorTarget {
    UiToggle,
    Slomo,
    Category(DebugCategory),
    DebugMaster,
}

#[derive(Component)]
pub struct AlsHudDebugSection;

#[derive(Component)]
pub struct AlsHudInputSection;

#[derive(Component)]
pub struct AlsHudRoot;

pub const INPUT_ROWS: [(&str, &str, &str); 14] = [
    ("Movement", "WASD", "Left Stick"),
    ("Camera", "Mouse", "Right Stick"),
    ("Sprint", "Hold Left Shift", "Hold B"),
    ("Roll", "Left Shift", "B"),
    ("Walk", "Left Alt", "RB"),
    ("Crouch", "Left Ctrl", "LSB"),
    ("Jump / Mantle", "Space", "A"),
    ("Aim", "RMB", "LB"),
    ("Ragdoll", "X", "X"),
    ("Rotation Mode", "1", "D-pad Left"),
    ("View Mode", "2", "D-pad Right"),
    ("Switch Shoulder", "C", "RSB"),
    ("Overlay Menu", "Q", "Y"),
    ("Cycle Overlay", "Mouse Wheel", "D-pad U / D"),
];

pub const DEBUG_ROWS: [(&str, &str, Option<DebugCategory>); 10] = [
    ("Toggle Slomo", "Z", None),
    ("Toggle Curves", "Shift + 1", Some(DebugCategory::Curves)),
    ("Toggle State", "Shift + 2", Some(DebugCategory::State)),
    ("Toggle Shapes", "Shift + 3", Some(DebugCategory::Shapes)),
    ("Toggle Traces", "Shift + 4", Some(DebugCategory::Traces)),
    (
        "Toggle Mantling",
        "Shift + 5",
        Some(DebugCategory::Mantling),
    ),
    (
        "Toggle Camera Curves",
        "Shift + 6",
        Some(DebugCategory::CameraCurves),
    ),
    (
        "Toggle Camera Shapes",
        "Shift + 7",
        Some(DebugCategory::CameraShapes),
    ),
    (
        "Toggle Camera Traces",
        "Shift + 8",
        Some(DebugCategory::CameraTraces),
    ),
    ("Toggle Debug", "Shift + 0", None),
];

pub fn slomo_time_scale(slomo_active: bool) -> f32 {
    if slomo_active { SLMO_TIME_SCALE } else { 1.0 }
}

pub struct AlsHudPlugin;

impl Plugin for AlsHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsHudState>()
            .add_systems(Startup, spawn_hud)
            .add_systems(
                Update,
                (
                    toggle_ui_system,
                    slomo_system,
                    refresh_colors_system.after(slomo_system),
                ),
            );
    }
}

fn spawn_hud(mut commands: Commands) {
    let root = commands
        .spawn((
            AlsHudRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(PANEL_RIGHT_MARGIN_PX),
                top: Val::Px(PANEL_TOP_MARGIN_PX),
                width: Val::Px(PANEL_WIDTH_PX),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(SECTION_GAP_PX),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(10),
        ))
        .id();

    spawn_header_section(&mut commands, root);
    spawn_debug_section(&mut commands, root);
    spawn_input_section(&mut commands, root);
}

fn section_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        ..default()
    }
}

fn cell_node(grow: f32, centered: bool) -> Node {
    Node {
        flex_grow: grow,
        flex_basis: Val::Px(0.0),
        margin: UiRect::left(Val::Px(CELL_GAP_PX)),
        align_items: if centered {
            AlignItems::Center
        } else {
            AlignItems::FlexStart
        },
        justify_content: if centered {
            JustifyContent::Center
        } else {
            JustifyContent::FlexStart
        },
        ..default()
    }
}

fn text_node(text: &str, size: f32, italic: bool, bold: bool, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            weight: if bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            },
            style: if italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            ..default()
        },
        TextColor(color),
    )
}

fn spawn_header_section(commands: &mut Commands, root: Entity) {
    let section = commands
        .spawn((section_node(), BackgroundColor(COLOR_BORDER_WEAK)))
        .id();

    let title_border = commands
        .spawn((
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
            BackgroundColor(COLOR_BORDER_STRONG),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(text_node(
                        "Advanced Locomotion System ",
                        FONT_SIZE_DEFAULT_PX,
                        true,
                        true,
                        COLOR_TEXT,
                    ));
                    row.spawn(text_node(
                        "Refactored",
                        FONT_SIZE_DEFAULT_PX,
                        true,
                        false,
                        COLOR_TITLE_ACCENT,
                    ));
                });
        })
        .id();

    let platform_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::top(Val::Px(ROW_GAP_PX)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                cell_node(LABEL_GROW, false),
                BackgroundColor(COLOR_BORDER_STRONG),
            ));
            row.spawn((
                cell_node(KEY_CELL_GROW, true),
                BackgroundColor(COLOR_BORDER_STRONG),
            ))
            .with_children(|cell| {
                cell.spawn(text_node(
                    "PC",
                    FONT_SIZE_DEFAULT_PX,
                    true,
                    false,
                    COLOR_TEXT,
                ));
            });
            row.spawn((
                cell_node(KEY_CELL_GROW, true),
                BackgroundColor(COLOR_BORDER_STRONG),
            ))
            .with_children(|cell| {
                cell.spawn(text_node(
                    "Xbox",
                    FONT_SIZE_DEFAULT_PX,
                    true,
                    false,
                    COLOR_TEXT,
                ));
            });
        })
        .id();

    let toggle_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::top(Val::Px(ROW_GAP_PX)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                cell_node(LABEL_GROW, false),
                BackgroundColor(COLOR_BORDER_STRONG),
            ))
            .with_children(|cell| {
                cell.spawn(text_node(
                    "Toggle UI",
                    FONT_SIZE_DEFAULT_PX,
                    false,
                    false,
                    COLOR_TEXT,
                ));
            });
            row.spawn((
                cell_node(KEY_CELL_GROW, true),
                BackgroundColor(COLOR_BORDER_STRONG),
            ))
            .with_children(|cell| {
                cell.spawn((
                    text_node("Tab", FONT_SIZE_DEFAULT_PX, true, false, COLOR_ENABLED),
                    AlsHudColorTarget::UiToggle,
                ));
            });
            row.spawn((
                cell_node(KEY_CELL_GROW, true),
                BackgroundColor(COLOR_BORDER_STRONG),
            ))
            .with_children(|cell| {
                cell.spawn((
                    text_node("View", FONT_SIZE_DEFAULT_PX, false, false, COLOR_ENABLED),
                    AlsHudColorTarget::UiToggle,
                ));
            });
        })
        .id();

    commands
        .entity(section)
        .add_child(title_border)
        .add_child(platform_row)
        .add_child(toggle_row);
    commands.entity(root).add_child(section);
}

fn spawn_debug_section(commands: &mut Commands, root: Entity) {
    let section = commands
        .spawn((
            section_node(),
            BackgroundColor(COLOR_BORDER_WEAK),
            AlsHudDebugSection,
        ))
        .id();

    let header = commands
        .spawn((
            Node {
                align_self: AlignSelf::Center,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(COLOR_BORDER_STRONG),
        ))
        .with_children(|parent| {
            parent.spawn(text_node(
                "Debug",
                FONT_SIZE_DEFAULT_PX,
                false,
                false,
                COLOR_TEXT,
            ));
        })
        .id();
    commands.entity(section).add_child(header);

    for (label, key, category) in DEBUG_ROWS {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(ROW_GAP_PX)),
                ..default()
            })
            .id();
        commands.entity(row).with_children(|row_parent| {
            row_parent
                .spawn((
                    cell_node(LABEL_GROW, false),
                    BackgroundColor(COLOR_BORDER_STRONG),
                ))
                .with_children(|cell| {
                    cell.spawn(text_node(
                        label,
                        FONT_SIZE_DEFAULT_PX,
                        false,
                        false,
                        COLOR_TEXT,
                    ));
                });
            if label == "Toggle Slomo" {
                row_parent
                    .spawn((
                        cell_node(KEY_CELL_GROW, true),
                        BackgroundColor(COLOR_BORDER_STRONG),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            text_node(key, FONT_SIZE_DEFAULT_PX, true, false, COLOR_DISABLED),
                            AlsHudColorTarget::Slomo,
                        ));
                    });
                row_parent
                    .spawn((
                        cell_node(KEY_CELL_GROW, true),
                        BackgroundColor(COLOR_BORDER_STRONG),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            text_node("Menu", FONT_SIZE_DEFAULT_PX, false, false, COLOR_DISABLED),
                            AlsHudColorTarget::Slomo,
                        ));
                    });
            } else {
                let target = match category {
                    Some(category) => AlsHudColorTarget::Category(category),
                    None => AlsHudColorTarget::DebugMaster,
                };
                row_parent
                    .spawn((
                        cell_node(DEBUG_KEY_CELL_GROW, true),
                        BackgroundColor(COLOR_BORDER_STRONG),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            text_node(key, FONT_SIZE_DEFAULT_PX, true, false, COLOR_DISABLED),
                            target,
                        ));
                    });
            }
        });
        commands.entity(section).add_child(row);
    }

    commands.entity(root).add_child(section);
}

fn spawn_input_section(commands: &mut Commands, root: Entity) {
    let section = commands
        .spawn((
            section_node(),
            BackgroundColor(COLOR_BORDER_WEAK),
            AlsHudInputSection,
        ))
        .id();

    let header = commands
        .spawn((
            Node {
                align_self: AlignSelf::Center,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(COLOR_BORDER_STRONG),
        ))
        .with_children(|parent| {
            parent.spawn(text_node(
                "Input",
                FONT_SIZE_DEFAULT_PX,
                false,
                false,
                COLOR_TEXT,
            ));
        })
        .id();
    commands.entity(section).add_child(header);

    for (label, pc, xbox) in INPUT_ROWS {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(ROW_GAP_PX)),
                ..default()
            })
            .with_children(|row_parent| {
                row_parent
                    .spawn((
                        cell_node(LABEL_GROW, false),
                        BackgroundColor(COLOR_BORDER_STRONG),
                    ))
                    .with_children(|cell| {
                        cell.spawn(text_node(
                            label,
                            FONT_SIZE_DEFAULT_PX,
                            false,
                            false,
                            COLOR_TEXT,
                        ));
                    });
                row_parent
                    .spawn((
                        cell_node(KEY_CELL_GROW, true),
                        BackgroundColor(COLOR_BORDER_STRONG),
                    ))
                    .with_children(|cell| {
                        cell.spawn(text_node(pc, FONT_SIZE_VALUE_PX, false, false, COLOR_TEXT));
                    });
                row_parent
                    .spawn((
                        cell_node(KEY_CELL_GROW, true),
                        BackgroundColor(COLOR_BORDER_STRONG),
                    ))
                    .with_children(|cell| {
                        cell.spawn(text_node(
                            xbox,
                            FONT_SIZE_VALUE_PX,
                            false,
                            false,
                            COLOR_TEXT,
                        ));
                    });
            })
            .id();
        commands.entity(section).add_child(row);
    }

    commands.entity(root).add_child(section);
}

fn toggle_ui_system(
    events: Query<&ActionEvents, With<Action<ToggleUiAction>>>,
    mut state: ResMut<AlsHudState>,
    mut debug_sections: Query<&mut Node, (With<AlsHudDebugSection>, Without<AlsHudInputSection>)>,
    mut input_sections: Query<&mut Node, With<AlsHudInputSection>>,
) {
    let pressed = events
        .iter()
        .any(|events| events.contains(ActionEvents::START));
    if !pressed {
        return;
    }
    state.ui_visible = !state.ui_visible;
    let display = if state.ui_visible {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut debug_sections {
        node.display = display;
    }
    for mut node in &mut input_sections {
        node.display = display;
    }
}

fn slomo_system(
    events: Query<&ActionEvents, With<Action<SlomoAction>>>,
    mut state: ResMut<AlsHudState>,
    mut time: ResMut<Time<Virtual>>,
) {
    let pressed = events
        .iter()
        .any(|events| events.contains(ActionEvents::START));
    if !pressed {
        return;
    }
    state.slomo_active = !state.slomo_active;
    time.set_relative_speed(slomo_time_scale(state.slomo_active));
}

fn refresh_colors_system(
    state: Res<AlsHudState>,
    show_debug: Option<Res<AlsShowDebugHudState>>,
    mut targets: Query<(&AlsHudColorTarget, &mut TextColor)>,
) {
    for (target, mut color) in &mut targets {
        let active = match *target {
            AlsHudColorTarget::UiToggle => state.ui_visible,
            AlsHudColorTarget::Slomo => state.slomo_active,
            AlsHudColorTarget::Category(category) => show_debug
                .as_ref()
                .map(|debug| debug.is_active(category))
                .unwrap_or(false),
            AlsHudColorTarget::DebugMaster => show_debug
                .as_ref()
                .map(|debug| debug.visible)
                .unwrap_or(false),
        };
        color.0 = AlsHudState::toggle_color(active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slomo_scale_matches_ue_dilation() {
        assert_eq!(slomo_time_scale(false), 1.0);
        assert_eq!(slomo_time_scale(true), SLMO_TIME_SCALE);
        assert_eq!(SLMO_TIME_SCALE, 0.15);
    }

    #[test]
    fn toggle_color_matches_ue_enabled_disabled() {
        assert_eq!(AlsHudState::toggle_color(true), COLOR_ENABLED);
        assert_eq!(AlsHudState::toggle_color(false), COLOR_DISABLED);
    }

    #[test]
    fn source_colors_remain_linear() {
        let disabled = COLOR_DISABLED.to_linear();
        let accent = COLOR_TITLE_ACCENT.to_linear();
        let weak = COLOR_BORDER_WEAK.to_linear();
        let strong = COLOR_BORDER_STRONG.to_linear();
        assert!((disabled.red - 0.5).abs() < 1e-6);
        assert!((accent.green - 0.666_667).abs() < 1e-6);
        assert!((weak.red - 0.015).abs() < 1e-6);
        assert!((weak.alpha - 0.25).abs() < 1e-6);
        assert!((strong.red - 0.015).abs() < 1e-6);
        assert!((strong.alpha - 0.5).abs() < 1e-6);
    }

    #[test]
    fn default_state_matches_ue_player_controller_defaults() {
        let state = AlsHudState::default();
        assert!(state.ui_visible);
        assert!(!state.slomo_active);
    }

    #[test]
    fn debug_rows_cover_all_categories_plus_master_and_slomo() {
        let categories: Vec<_> = DEBUG_ROWS.iter().filter_map(|(_, _, c)| *c).collect();
        assert_eq!(categories.len(), 8);
        for category in DebugCategory::ALL {
            assert!(categories.contains(&category), "{category:?} missing");
        }
        assert_eq!(DEBUG_ROWS.len(), 10);
        assert_eq!(DEBUG_ROWS[0].0, "Toggle Slomo");
        assert_eq!(DEBUG_ROWS[9].0, "Toggle Debug");
    }

    #[test]
    fn input_rows_match_ue_panel() {
        assert_eq!(INPUT_ROWS.len(), 14);
        assert_eq!(INPUT_ROWS[0], ("Movement", "WASD", "Left Stick"));
        assert_eq!(
            INPUT_ROWS[13],
            ("Cycle Overlay", "Mouse Wheel", "D-pad U / D")
        );
    }
}
