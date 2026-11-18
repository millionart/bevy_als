use std::time::Duration;

use avian3d::prelude::{Collider, CollisionLayers, LayerMask, RigidBody};
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::ExitCondition;
use bevy_als::als::character::AlsBirthAnimationPending;
use bevy_als::character_assembly::AlsCharacterAssembly;
use bevy_als::{
    AlsCharacter, AlsCharacterBlueprint, AlsCollisionLayer, AlsControlSchemeConfig,
    AlsMovementParams, AlsPlugin, AlsStateRequest, LocomotionState, Stance, spawn_als_character,
};
use bevy_enhanced_input::prelude::{EnhancedInputPlugin, EnhancedInputSystems};
use bevy_gameplay_tag::GameplayTagsManager;

fn main() {
    let asset_path = std::env::args()
        .nth(1)
        .expect("Pass the host asset directory");
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_path,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            }),
        EnhancedInputPlugin,
        AlsPlugin::physics_plugins(),
        AlsPlugin,
    ));
    assert!(
        app.world()
            .resource::<GameplayTagsManager>()
            .has_tag_name("Als.OverlayMode.Default"),
        "Load ALS tags from the host AssetPlugin directory"
    );
    println!("ALS plugins and tags loaded");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )))
    .add_systems(Startup, setup)
    .add_systems(
        PreUpdate,
        (|mut input: ResMut<ButtonInput<KeyCode>>| input.press(KeyCode::KeyW))
            .after(InputSystems)
            .before(EnhancedInputSystems::Prepare),
    )
    .add_systems(PostUpdate, verify)
    .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut params: ResMut<Assets<AlsMovementParams>>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(64.0, 0.2, 64.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
        CollisionLayers::new(
            [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility],
            LayerMask::ALL,
        ),
    ));
    spawn_als_character(
        &mut commands,
        &assets,
        &mut params,
        &mut configs,
        AlsCharacterBlueprint {
            spawn_camera: true,
            ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 1.5, 0.0))
        },
    );
}

fn verify(
    actor: Single<
        (
            &Transform,
            &LocomotionState,
            &Stance,
            &AlsCharacterAssembly,
            &mut AlsStateRequest,
        ),
        (With<AlsCharacter>, Without<AlsBirthAnimationPending>),
    >,
    cameras: Query<&Camera3d>,
    mut start: Local<Option<Vec3>>,
    mut exit: MessageWriter<AppExit>,
) {
    let (transform, mode, stance, assembly, mut request) = actor.into_inner();
    if *mode != LocomotionState::Grounded {
        return;
    }
    if start.is_none() {
        assert!(
            assembly
                .camera
                .is_some_and(|camera| cameras.contains(camera))
        );
        *start = Some(transform.translation);
        request.stance = Some(Stance::Crouching);
        println!("ALS avatar and camera ready; crouch requested");
        return;
    }
    let distance = (transform.translation - start.unwrap()).xz().length();
    if *stance == Stance::Crouching && distance >= 0.25 {
        println!("ALS host input moved the crouched character {distance:.3} m");
        exit.write(AppExit::Success);
    }
}
