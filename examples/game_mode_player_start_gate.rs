use std::num::NonZeroU8;

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use bevy_als::als::character::{AlsCharacter, AlsCharacterPlugin};
use bevy_als::als::control_scheme::AlsControlScheme;
use bevy_als::als::input::AlsInputPlugin;
use bevy_als::als::player_start::{
    AlsPlayerStart, AlsPlayerStartOccupancy, AlsPlayerStartPlugin, AlsPlayerStartReady,
};
use bevy_als::extensions::AlsExtPlugin;
use bevy_als::game_mode::{
    AlsGameModePlugin, AlsPlayerAdmission, AlsPlayerAdmissionComplete, AlsScenePhysicsReady,
    request_als_player,
};

#[derive(Resource, Default)]
struct GateState {
    frame: u32,
    portal: Option<Entity>,
    vertical: Option<Entity>,
    free: Option<Entity>,
    regular_request: Option<Entity>,
    portal_request: Option<Entity>,
    vertical_request: Option<Entity>,
    delayed_collider_spawned: bool,
    barrier_released: bool,
    portal_ready_adjusted: bool,
    vertical_ready_adjusted: bool,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            PhysicsPlugins::new(FixedPostUpdate),
            TnuaAvian3dPlugin::new(FixedUpdate),
            TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
            EnhancedInputPlugin,
            AlsInputPlugin,
            AlsCharacterPlugin,
            AlsPlayerStartPlugin,
            AlsGameModePlugin,
            AnimationGraphPlugin::default(),
            AlsExtPlugin,
        ))
        .init_resource::<GateState>()
        .init_resource::<AlsScenePhysicsReady>()
        .add_systems(Startup, setup)
        .add_systems(Update, drive_gate)
        .run();
}

fn setup(mut commands: Commands, mut state: ResMut<GateState>) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(30.0, 0.3, 30.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
    ));
    let portal = commands
        .spawn((
            Name::new("Portal PlayerStart"),
            AlsPlayerStart {
                tag: Some("Portal".into()),
                snap_to_ground: false,
            },
            Transform::from_xyz(0.0, 0.919, 0.0).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.0,
                0.35,
                -0.25,
            )),
        ))
        .id();
    let vertical = commands
        .spawn((
            Name::new("Vertical PlayerStart"),
            AlsPlayerStart {
                tag: Some("Vertical".into()),
                snap_to_ground: false,
            },
            Transform::from_xyz(-6.0, 0.919, 0.0).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                -0.45,
                -0.3,
                0.2,
            )),
        ))
        .id();
    let free = commands
        .spawn((
            Name::new("Free PlayerStart"),
            AlsPlayerStart {
                tag: None,
                snap_to_ground: false,
            },
            Transform::from_xyz(4.0, 0.919, 0.0).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.7,
                0.4,
                -0.2,
            )),
        ))
        .id();
    let mut regular = AlsPlayerAdmission::new();
    regular.spawn_input = false;
    let regular_request = request_als_player(&mut commands, regular);
    state.portal = Some(portal);
    state.vertical = Some(vertical);
    state.free = Some(free);
    state.regular_request = Some(regular_request);
}

#[allow(clippy::type_complexity)]
fn drive_gate(
    mut commands: Commands,
    mut state: ResMut<GateState>,
    mut barrier: ResMut<AlsScenePhysicsReady>,
    starts: Query<(Entity, &AlsPlayerStartReady)>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    characters: Query<(&Transform, &Collider), With<AlsCharacter>>,
    spatial: SpatialQuery,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.frame == 4 && !state.delayed_collider_spawned {
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(1.2, 1.4, 1.2),
            Transform::from_xyz(0.0, 0.7, 0.0),
        ));
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(2.0, 0.5, 2.0),
            Transform::from_xyz(-6.0, 0.7, 0.0),
        ));
        state.delayed_collider_spawned = true;
        println!("[BCHAR19][DELAY] collider_spawned=true barrier=false");
    }
    if !state.barrier_released {
        if let Some(portal) = state.portal
            && let Ok((_, ready)) = starts.get(portal)
            && ready.occupancy == AlsPlayerStartOccupancy::OccupiedAdjusted
            && ready.teleport_adjustment.xz().length() > 1e-4
        {
            state.portal_ready_adjusted = true;
        }
        if let Some(vertical) = state.vertical
            && let Ok((_, ready)) = starts.get(vertical)
            && ready.occupancy == AlsPlayerStartOccupancy::OccupiedAdjusted
            && ready.teleport_adjustment.y.abs() > 1e-4
        {
            state.vertical_ready_adjusted = true;
        }
        let regular_spawned_early = state
            .regular_request
            .and_then(|entity| admissions.get(entity).ok())
            .is_some();
        if regular_spawned_early {
            println!("[BCHAR19][VERDICT] barrier_hold=false -> FAIL");
            exit.write(AppExit::Error(NonZeroU8::new(19).unwrap()));
            return;
        }
        if state.portal_ready_adjusted && state.vertical_ready_adjusted {
            barrier.ready = true;
            state.barrier_released = true;
            println!(
                "[BCHAR19][BARRIER] portal_xy=true vertical_z=true starts={} -> PASS",
                starts.iter().count()
            );
        }
        return;
    }
    let Some(regular_request) = state.regular_request else {
        return;
    };
    let Ok(regular) = admissions.get(regular_request) else {
        return;
    };
    let Some(free) = state.free else {
        return;
    };
    let regular_transform = characters
        .get(regular.character)
        .map(|(transform, _)| *transform)
        .ok();
    let regular_pass = regular.selection.entity == free
        && regular.selection.fit.occupancy == AlsPlayerStartOccupancy::Unoccupied
        && regular_transform.is_some_and(|transform| {
            let (_, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
            pitch.abs() <= 1e-5 && roll.abs() <= 1e-5
        });
    if !regular_pass {
        println!("[BCHAR19][VERDICT] unoccupied_yaw_only=false -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(20).unwrap()));
        return;
    }
    if state.portal_request.is_none() {
        let mut request = AlsPlayerAdmission::new();
        request.spawn_input = false;
        request.incoming_start_tag = Some("Portal".into());
        state.portal_request = Some(request_als_player(&mut commands, request));
        println!("[BCHAR19][REGULAR] unoccupied=true yaw_only=true -> PASS");
        return;
    }
    let Some(portal_request) = state.portal_request else {
        return;
    };
    let Ok(portal) = admissions.get(portal_request) else {
        return;
    };
    if state.vertical_request.is_none() {
        let mut request = AlsPlayerAdmission::new();
        request.spawn_input = false;
        request.incoming_start_tag = Some("Vertical".into());
        state.vertical_request = Some(request_als_player(&mut commands, request));
    }
    let Some(vertical_request) = state.vertical_request else {
        return;
    };
    let Ok(vertical) = admissions.get(vertical_request) else {
        return;
    };
    let no_encroachment = [portal.character, vertical.character]
        .into_iter()
        .all(|character| {
            characters
                .get(character)
                .is_ok_and(|(transform, collider)| {
                    spatial
                        .shape_intersections(
                            collider,
                            transform.translation,
                            transform.rotation,
                            &SpatialQueryFilter::from_excluded_entities([character]),
                        )
                        .is_empty()
                })
        });
    let pass = portal.selection.tag.as_deref() == Some("Portal")
        && portal.selection.fit.occupancy == AlsPlayerStartOccupancy::OccupiedAdjusted
        && vertical.selection.tag.as_deref() == Some("Vertical")
        && vertical.selection.fit.occupancy == AlsPlayerStartOccupancy::OccupiedAdjusted
        && no_encroachment;
    println!(
        "[BCHAR19][VERDICT] barrier=true unoccupied=true portal_xy={} vertical_z={} no_encroachment={} -> {}",
        portal.selection.fit.teleport_adjustment.xz().length() > 1e-4,
        vertical.selection.fit.teleport_adjustment.y.abs() > 1e-4,
        no_encroachment,
        if pass { "PASS" } else { "FAIL" }
    );
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(NonZeroU8::new(21).unwrap()));
    }
}
