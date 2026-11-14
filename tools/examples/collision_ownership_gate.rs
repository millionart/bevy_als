use avian3d::prelude::CollisionLayers;
use bevy::prelude::*;
use bevy_als::als::character::{
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, set_collision_layer_owner,
};

#[derive(Resource)]
struct GateRequest {
    character: Entity,
    owner: AlsCollisionLayerOwner,
    active: bool,
    pending: bool,
}

fn main() {
    let custom = CollisionLayers::from_bits(0b0010, 0b1100);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_systems(Update, apply_request);
    let character = app.world_mut().spawn((AlsCharacter, custom)).id();
    app.insert_resource(GateRequest {
        character,
        owner: AlsCollisionLayerOwner::Mantle,
        active: false,
        pending: false,
    });

    request_owner(&mut app, AlsCollisionLayerOwner::Mantle, true);
    let mantle_disabled = current_layers(&app, character) == Some(CollisionLayers::NONE)
        && ownership(&app, character).mantle;

    request_owner(&mut app, AlsCollisionLayerOwner::Ragdoll, true);
    let ragdoll_disabled = current_layers(&app, character) == Some(CollisionLayers::NONE)
        && ownership(&app, character).mantle
        && ownership(&app, character).ragdoll;

    request_owner(&mut app, AlsCollisionLayerOwner::Mantle, false);
    let mantle_release_preserved_ragdoll = current_layers(&app, character)
        == Some(CollisionLayers::NONE)
        && !ownership(&app, character).mantle
        && ownership(&app, character).ragdoll;

    request_owner(&mut app, AlsCollisionLayerOwner::Ragdoll, false);
    let restored_custom =
        current_layers(&app, character) == Some(custom) && !ownership(&app, character).active();

    app.world_mut()
        .entity_mut(character)
        .insert(CollisionLayers::NONE);
    request_owner(&mut app, AlsCollisionLayerOwner::Mantle, true);
    request_owner(&mut app, AlsCollisionLayerOwner::Mantle, false);
    let pre_disabled_preserved = current_layers(&app, character) == Some(CollisionLayers::NONE)
        && !ownership(&app, character).active();

    let passed = mantle_disabled
        && ragdoll_disabled
        && mantle_release_preserved_ragdoll
        && restored_custom
        && pre_disabled_preserved;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.21][VERDICT] mantle_disabled={} ragdoll_disabled={} mantle_release_preserved_ragdoll={} restored_custom={} pre_disabled_preserved={} final_layers={:?} -> {verdict}",
        mantle_disabled,
        ragdoll_disabled,
        mantle_release_preserved_ragdoll,
        restored_custom,
        pre_disabled_preserved,
        current_layers(&app, character),
    );
    if !passed {
        std::process::exit(21);
    }
}

fn apply_request(
    mut commands: Commands,
    mut request: ResMut<GateRequest>,
    mut characters: Query<
        (
            Entity,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
) {
    if !request.pending {
        return;
    }
    let Ok((entity, layers, mut ownership)) = characters.get_mut(request.character) else {
        return;
    };
    set_collision_layer_owner(
        &mut commands,
        entity,
        &mut ownership,
        layers.copied(),
        request.owner,
        request.active,
    );
    request.pending = false;
}

fn request_owner(app: &mut App, owner: AlsCollisionLayerOwner, active: bool) {
    {
        let mut request = app.world_mut().resource_mut::<GateRequest>();
        request.owner = owner;
        request.active = active;
        request.pending = true;
    }
    app.update();
}

fn current_layers(app: &App, character: Entity) -> Option<CollisionLayers> {
    app.world().get::<CollisionLayers>(character).copied()
}

fn ownership(app: &App, character: Entity) -> AlsCollisionLayerOwnership {
    *app.world()
        .get::<AlsCollisionLayerOwnership>(character)
        .unwrap()
}
