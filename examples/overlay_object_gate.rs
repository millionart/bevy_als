use std::num::NonZeroU8;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;

use als_core::character::AlsCharacter;
use als_core::state::OverlayState;
use bevy_als::character_assembly::AlsCharacterAssembly;
use bevy_als::overlay_object::{
    AlsOverlayObject, LEFT_GUN_BONE, OverlayObjectKind, OverlayObjectPlugin, RIGHT_GUN_BONE,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            bevy_als::playable_character::AlsPlayableCharacterPlugin,
            OverlayObjectPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, verify_overlay_objects)
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    phase: u8,
    frames: u32,
}

#[derive(Resource)]
struct BoneTargets {
    right: Entity,
    left: Entity,
}

fn setup(mut commands: Commands) {
    let avatar = commands
        .spawn((Name::new("avatar"), Transform::IDENTITY))
        .id();
    let right = commands
        .spawn((Name::new(RIGHT_GUN_BONE), Transform::IDENTITY))
        .id();
    let left = commands
        .spawn((Name::new(LEFT_GUN_BONE), Transform::IDENTITY))
        .id();
    commands.entity(avatar).add_children(&[right, left]);
    commands.insert_resource(BoneTargets { right, left });
    commands.spawn((
        AlsCharacter,
        OverlayState::Rifle,
        AlsCharacterAssembly {
            avatar,
            camera: None,
        },
    ));
}

fn verify_overlay_objects(
    mut state: Local<GateState>,
    mut characters: Query<&mut OverlayState, With<AlsCharacter>>,
    props: Query<(&AlsOverlayObject, &ChildOf)>,
    targets: Res<BoneTargets>,
    mut exit: MessageWriter<AppExit>,
) {
    state.frames += 1;
    let Ok(mut overlay) = characters.single_mut() else {
        return;
    };
    let all = props.iter().collect::<Vec<_>>();
    if state.frames > 240 {
        eprintln!(
            "[BCHAR8][VERDICT] timeout phase={} props={} -> FAIL",
            state.phase,
            all.len()
        );
        exit.write(AppExit::Error(NonZeroU8::new(15).unwrap()));
        return;
    }
    match state.phase {
        0 => {
            let pass = all.len() == 1
                && all[0].0.overlay == OverlayState::Rifle
                && all[0].0.definition.kind == OverlayObjectKind::Skeletal
                && all[0].0.attachment_bone == RIGHT_GUN_BONE
                && all[0].1.0 == targets.right;
            if !pass {
                return;
            }
            *overlay = OverlayState::Torch;
            state.phase = 1;
        }
        1 => {
            let pass = all.len() == 1
                && all[0].0.overlay == OverlayState::Torch
                && all[0].0.definition.kind == OverlayObjectKind::Static
                && all[0].0.attachment_bone == LEFT_GUN_BONE
                && all[0].1.0 == targets.left;
            if !pass {
                return;
            }
            *overlay = OverlayState::Default;
            state.phase = 2;
        }
        _ => {
            if !all.is_empty() {
                return;
            }
            println!(
                "[BCHAR8][VERDICT] rifle=Skeletal/right_gun_bone torch=Static/left_gun_bone default_clears=true prop_count=0 -> PASS"
            );
            exit.write(AppExit::Success);
        }
    }
}
