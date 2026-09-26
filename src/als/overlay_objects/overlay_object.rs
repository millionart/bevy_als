use crate::als::character::AlsCharacter;
use crate::als::mantle::MantleKind;
use crate::als::state::{AlsNotifyLocomotionAction, OverlayState};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance},
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    context::system_resources::SystemResources,
    plugin::AnimationGraphSet,
    systems::run_animation_player,
};
use crate::extensions::{overlay_materials::{AvatarMaterialApplied, AvatarMaterialOverride},
    post_physics_animation::{ManualAnimationPlayer, PostPhysicsAnimationSet}};
use serde::Deserialize;
use std::collections::HashMap;

use crate::character_assembly::AlsCharacterAssembly;

pub const LEFT_GUN_BONE: &str = "VB hand_l_to_ik_hand_gun";
pub const RIGHT_GUN_BONE: &str = "VB hand_r_to_ik_hand_gun";

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ObjectMaterial {
    pub linear_rgba: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ObjectDefinition {
    pub kind: OverlayObjectKind,
    pub mesh: String,
    pub scene: Option<String>,
    pub socket_name: String,
    pub socket_bone: String,
    pub use_left_gun_bone: bool,
    pub socket_translation: Vec3,
    pub socket_rotation: Quat,
    pub socket_scale: Vec3,
    pub materials: Vec<ObjectMaterial>,
}

#[derive(Clone, Deserialize)]
pub struct OverlayDefinition {
    pub graph: String,
    pub blend_in: f32,
    pub blend_out: f32,
    pub object: Option<ObjectDefinition>,
    #[serde(skip)]
    pub(crate) graph_handle: Handle<bevy_animation_graph::core::animation_graph::AnimationGraph>,
}

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct OverlayLibrary(pub HashMap<OverlayState, OverlayDefinition>);

#[derive(Resource)]
pub struct OverlayLibraryHandle(pub Handle<OverlayLibrary>);

impl FromWorld for OverlayLibraryHandle {
    fn from_world(world: &mut World) -> Self {
        Self(world.resource::<AssetServer>().load("ALS/Character/overlay_actor.runtime.json"))
    }
}

#[derive(Default, TypePath)]
struct OverlayLibraryLoader;

impl bevy::asset::AssetLoader for OverlayLibraryLoader {
    type Asset = OverlayLibrary;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(&self, reader: &mut dyn bevy::asset::io::Reader, _: &(), context: &mut bevy::asset::LoadContext<'_>) -> Result<OverlayLibrary, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let mut library: OverlayLibrary = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        for (tag, definition) in &mut library.0 {
            if tag.as_str().is_empty() || definition.graph.is_empty() || !definition.blend_in.is_finite() || !definition.blend_out.is_finite() {
                return Err(std::io::Error::other("Overlay tag, graph or blend duration is invalid"));
            }
            definition.graph_handle = context.load(definition.graph.clone());
        }
        Ok(library)
    }

    fn extensions(&self) -> &[&str] { &["json"] }
}

pub struct OverlayLibraryPlugin;

impl Plugin for OverlayLibraryPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<OverlayLibrary>().init_asset_loader::<OverlayLibraryLoader>().init_resource::<OverlayLibraryHandle>();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum OverlayObjectKind {
    Static,
    Skeletal,
}

pub fn overlay_object_spec(library: &OverlayLibrary, overlay: OverlayState) -> Option<&ObjectDefinition> {
    library.0.get(&overlay)?.object.as_ref()
}

pub fn attachment_bone(spec: &ObjectDefinition, use_gun_bones: bool) -> &str {
    if use_gun_bones {
        if spec.use_left_gun_bone {
            LEFT_GUN_BONE
        } else {
            RIGHT_GUN_BONE
        }
    } else {
        &spec.socket_bone
    }
}

pub fn overlay_object_visible(
    library: &OverlayLibrary,
    overlay: OverlayState,
    ragdolling: bool,
    mantle_active: bool,
    mantle_kind: Option<MantleKind>,
) -> bool {
    overlay_object_spec(library, overlay).is_some()
        && !ragdolling
        && !(mantle_active && mantle_kind.is_some_and(MantleKind::is_high))
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsOverlayObjectConfig {
    pub use_gun_bones: bool,
}

impl Default for AlsOverlayObjectConfig {
    fn default() -> Self {
        Self {
            use_gun_bones: true,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct AlsOverlayObject {
    pub character: Entity,
    pub content: Entity,
    pub overlay: OverlayState,
    pub definition: ObjectDefinition,
    pub attachment_bone: String,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsOverlayObjectState {
    pub components: [Entity; 2],
    pub prop: Option<Entity>,
    pub applied_overlay: Option<OverlayState>,
    pub visible: bool,
}

pub struct OverlayObjectPlugin;

impl Plugin for OverlayObjectPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<OverlayLibraryPlugin>() { app.add_plugins(OverlayLibraryPlugin); }
        app.add_systems(
            Update,
            (ensure_overlay_object_state, sync_overlay_object).chain(),
        ).add_systems(PostUpdate, initialize_object_players.before(AnimationGraphSet::PrePhysics))
            .add_systems(PostUpdate, update_object_animation.after(PostPhysicsAnimationSet::Evaluate)
                .before(AnimationGraphSet::PostPhysics));
    }
}

fn ensure_overlay_object_state(
    mut commands: Commands,
    mut characters: Query<(Entity, &AlsCharacterAssembly, Option<&AlsOverlayObjectConfig>, Option<&mut AlsOverlayObjectState>), With<AlsCharacter>>,
    entities: Query<()>,
) {
    for (entity, assembly, config, state) in &mut characters {
        let components = [(0, "OverlayStaticMesh"), (1, "OverlaySkeletalMesh")].map(|(index, name)| {
            state.as_ref().map(|state| state.components[index]).filter(|component| entities.contains(*component))
                .unwrap_or_else(|| commands.spawn((Name::new(name), Transform::IDENTITY, Visibility::Inherited, ChildOf(assembly.avatar))).id())
        });
        if let Some(mut state) = state {
            if state.components != components { state.components = components; }
        } else {
            commands.entity(entity).insert(AlsOverlayObjectState { components, prop: None, applied_overlay: None, visible: false });
        }
        if config.is_none() { commands.entity(entity).insert(AlsOverlayObjectConfig::default()); }
    }
}

fn find_named_descendant(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if names
            .get(entity)
            .is_ok_and(|candidate| candidate.as_str() == name)
        {
            return Some(entity);
        }
        if let Ok(descendants) = children.get(entity) {
            stack.extend(descendants.iter());
        }
    }
    None
}

fn sync_overlay_object(
    mut commands: Commands,
    assets: Res<AssetServer>,
    library_handle: Res<OverlayLibraryHandle>,
    libraries: Res<Assets<OverlayLibrary>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    props: Query<&AlsOverlayObject>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut characters: Query<
        (
            Entity,
            &OverlayState,
            Option<&AlsNotifyLocomotionAction>,
            Option<&crate::als::mantle::AlsMantleState>,
            &AlsCharacterAssembly,
            Ref<AlsOverlayObjectConfig>,
            &mut AlsOverlayObjectState,
        ),
        With<AlsCharacter>,
    >,
) {
    let Some(library) = libraries.get(&library_handle.0) else { return; };
    for (character, overlay, action, mantle, assembly, config, mut state) in &mut characters {
        let visible = overlay_object_visible(
            library,
            *overlay,
            action == Some(&AlsNotifyLocomotionAction::Ragdolling),
            mantle.is_some_and(|state| state.active),
            mantle.map(|state| state.kind),
        );
        let desired_overlay = visible.then_some(*overlay);
        let attachment = desired_overlay.and_then(|overlay| {
            let spec = overlay_object_spec(library, overlay)?;
            let bone_name = attachment_bone(spec, config.use_gun_bones);
            let bone = find_named_descendant(assembly.avatar, bone_name, &children, &names)?;
            Some((overlay, spec, bone_name, bone))
        });
        let prop_exists = state.prop.is_some_and(|prop| props.get(prop).is_ok());
        let definition_matches = state.prop.and_then(|prop| props.get(prop).ok())
            .is_some_and(|object| overlay_object_spec(library, *overlay) == Some(&object.definition));
        if state.applied_overlay == desired_overlay
            && state.visible == visible
            && prop_exists
            && definition_matches
            && attachment.is_some_and(|(_, _, _, bone)| state.components.iter().all(|component| parents.get(*component).is_ok_and(|parent| parent.parent() == bone)))
            && !config.is_changed()
        {
            continue;
        }
        if state.applied_overlay == desired_overlay
            && desired_overlay.is_none()
            && state.prop.is_none()
            && !config.is_changed()
        {
            continue;
        }
        let retained = attachment.and_then(|(_, spec, _, _)| {
            let object = props.get(state.prop?).ok()?;
            (object.definition.kind == spec.kind && object.definition.mesh == spec.mesh
                && object.definition.scene == spec.scene).then_some(object.content)
        });
        if let Some(prop) = state.prop.take()
            && let Ok(object) = props.get(prop)
            && retained.is_none()
        {
            commands.entity(object.content).despawn();
            commands.entity(prop).remove::<(AlsOverlayObject, AvatarMaterialOverride, AvatarMaterialApplied)>();
        }
        state.applied_overlay = attachment.map(|(overlay, _, _, _)| overlay);
        state.visible = attachment.is_some();
        let Some((overlay, spec, bone_name, bone)) = attachment else { continue; };
        let definition = spec;
        let prop = state.components[usize::from(spec.kind == OverlayObjectKind::Skeletal)];
        let transform = if !config.use_gun_bones {
            Transform { translation: definition.socket_translation, rotation: definition.socket_rotation, scale: definition.socket_scale }
        } else { Transform::IDENTITY };
        let content = retained.unwrap_or_else(|| {
            let mut content = commands.spawn((Name::new(format!("AlsOverlayProp::{overlay:?}")), Transform::IDENTITY, Visibility::Inherited, ChildOf(prop)));
            if let Some(scene) = &spec.scene {
                content.insert(AnimatedSceneHandle::new(assets.load(scene.clone())));
            } else {
                content.insert(WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(spec.mesh.clone()))));
            }
            content.id()
        });
        for component in state.components { commands.entity(component).insert((ChildOf(bone), transform)); }
        commands.entity(prop).insert(AlsOverlayObject {
            character, content, overlay, definition: spec.clone(), attachment_bone: bone_name.to_owned(),
        });
        if retained.is_none() || props.get(prop).is_ok_and(|object| object.definition.materials != definition.materials) {
            let handles = definition.materials.iter().map(|value| materials.add(StandardMaterial {
                base_color: Color::linear_rgba(value.linear_rgba[0], value.linear_rgba[1], value.linear_rgba[2], value.linear_rgba[3]),
                metallic: value.metallic, perceptual_roughness: value.roughness, reflectance: value.reflectance, ..default()
            })).collect();
            commands.entity(prop).remove::<AvatarMaterialApplied>().insert(AvatarMaterialOverride(handles));
        }
        state.prop = Some(prop);
    }
}

fn initialize_object_players(
    mut commands: Commands,
    objects: Query<&AlsOverlayObject>,
    scenes: Query<&AnimatedSceneInstance>,
    players: Query<(), (With<AnimationGraphPlayer>, Without<ManualAnimationPlayer>)>,
) {
    for object in &objects {
        if let Ok(scene) = scenes.get(object.content)
            && players.contains(scene.player_entity()) {
            commands.entity(scene.player_entity()).insert(ManualAnimationPlayer);
        }
    }
}

fn update_object_animation(
    source_inputs: Query<&crate::extensions::SourceAnimationInputs>,
    time: Res<Time>,
    assets: Res<AssetServer>,
    objects: Query<&AlsOverlayObject>,
    characters: Query<&AlsCharacterAssembly>,
    scenes: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    resources: SystemResources,
) {
    for object in &objects {
        let Ok(scene) = scenes.get(object.content) else { continue; };
        let amount = characters.get(object.character).ok()
            .and_then(|owner| scenes.get(owner.avatar).ok())
            .and_then(|owner| players.get(owner.player_entity()).ok())
            .and_then(|player| {
                let AnimationSource::Graph(graph) = player.get_animation_source() else { return None; };
                let graph = resources.animation_graph_assets.get(graph)?;
                let node = crate::animation_context_projection::root_pose_source_node(graph)?;
                crate::extensions::player_animation_context_state(player, node)?.curves.get("PoseAiming").copied()
            }).unwrap_or(0.0);
        let Ok(mut player) = players.get_mut(scene.player_entity()) else { continue; };
        if let AnimationSource::Graph(graph) = player.get_animation_source()
            && !assets.is_loaded_with_dependencies(graph) { continue; }
        player.set_global_input_data("draw_amount", amount);
        source_inputs.get(scene.player_entity()).unwrap().evaluate(scene.player_entity(), || run_animation_player(scene.player_entity(), player, &time, &resources));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) static SOURCE_LIBRARY: std::sync::LazyLock<OverlayLibrary> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../../../assets/ALS/Character/overlay_actor.runtime.json")).unwrap()
    });

    #[test]
    fn retained_character_rebinds_overlay_across_avatar_and_bone_replacements() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::character_assembly::{AlsCharacterAvatar, AlsCharacterBlueprint, ALS_LINKED_ANIMATED_SCENE, attach_avatar, spawn_als_character};
        fn below(world: &World, mut entity: Entity, root: Entity) -> bool {
            while let Some(parent) = world.get::<ChildOf>(entity) {
                entity = parent.parent();
                if entity == root { return true; }
            }
            false
        }
        fn ready(app: &mut App, avatar: Entity) -> Entity {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
            loop {
                app.update();
                if let Some(instance) = app.world().get::<AnimatedSceneInstance>(avatar)
                    && let Some(player) = app.world().get::<AnimationGraphPlayer>(instance.player_entity())
                    && player.get_default_output_pose().is_some() {
                    return instance.player_entity();
                }
                assert!(std::time::Instant::now() < deadline, "Replacement avatar did not evaluate a pose");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        OverlayObjectPlugin.build(&mut app);
        let character = app.world_mut().run_system_once(|mut commands: Commands, assets: Res<AssetServer>,
            mut params: ResMut<Assets<crate::als::params::AlsMovementParams>>, mut configs: ResMut<Assets<crate::als::control_scheme::AlsControlSchemeConfig>>| {
            spawn_als_character(&mut commands, &assets, &mut params, &mut configs,
                AlsCharacterBlueprint { spawn_input: false, ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 3.0, 0.0)) })
        }).unwrap();
        app.world_mut().entity_mut(character).insert((OverlayState::Bow, avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled));
        let mut avatar = app.world().get::<AlsCharacterAssembly>(character).unwrap().avatar;
        ready(&mut app, avatar);
        app.update();
        for delete_first in [false, true, false, true] {
            let old_avatar = avatar;
            let old_player = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
            let old_components = app.world().get::<AlsOverlayObjectState>(character).unwrap().components;
            if delete_first {
                app.world_mut().despawn(old_avatar);
                app.update();
            } else {
                app.world_mut().entity_mut(old_avatar).remove::<AlsCharacterAvatar>();
                app.world_mut().get_mut::<AnimationGraphPlayer>(old_player).unwrap().pause();
            }
            avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, assets: Res<AssetServer>| {
                attach_avatar(&mut commands, &assets, character, ALS_LINKED_ANIMATED_SCENE)
            }, character).unwrap();
            app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
            ready(&mut app, avatar);
            app.update();
            app.update();
            let state = *app.world().get::<AlsOverlayObjectState>(character).unwrap();
            assert!(state.components.iter().all(|component| app.world().get_entity(*component).is_ok()), "Replacement retained destroyed mount entities");
            assert!(state.components.iter().all(|component| below(app.world(), *component, avatar)), "Overlay mounts remained on the old avatar");
            if !delete_first {
                assert_eq!(state.components, old_components);
                app.world_mut().despawn(old_avatar);
            }
            assert!(app.world().get_entity(old_player).is_err());
            for overlay in [OverlayState::Rifle, OverlayState::Bow] {
                *app.world_mut().get_mut::<OverlayState>(character).unwrap() = overlay;
                for _ in 0..4 { app.update(); }
                let state = app.world().get::<AlsOverlayObjectState>(character).unwrap();
                let object = app.world().get::<AlsOverlayObject>(state.prop.unwrap()).unwrap();
                assert_eq!(object.overlay, overlay);
                assert!(below(app.world(), object.content, avatar));
                if overlay == OverlayState::Bow {
                    let content = object.content;
                    let player_entity = ready(&mut app, content);
                    let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
                    assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
                }
            }
        }
        let mounts = app.world().get::<AlsOverlayObjectState>(character).unwrap().components;
        let replacement_bone = app.world_mut().spawn((Name::new(LEFT_GUN_BONE), Transform::IDENTITY, ChildOf(avatar))).id();
        app.update();
        assert!(mounts.iter().all(|component| app.world().get::<ChildOf>(*component).unwrap().parent() == replacement_bone), "Unchanged overlay ignored replacement bone identity");
        app.world_mut().despawn(character);
        app.update();
        assert!(mounts.iter().all(|component| app.world().get_entity(*component).is_err()));
    }

    #[test]
    fn bow_actor_installs_the_object_animation_scene() {
        use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        OverlayObjectPlugin.build(&mut app);
        let avatar = app.world_mut().spawn(Transform::IDENTITY).id();
        app.world_mut().spawn((Name::new(LEFT_GUN_BONE), Transform::IDENTITY, ChildOf(avatar)));
        let character = app.world_mut().spawn((AlsCharacter, OverlayState::Bow, AlsCharacterAssembly { avatar, camera: None })).id();
        app.update();
        app.update();
        let prop = app.world().get::<AlsOverlayObjectState>(character).unwrap().prop.unwrap();
        let mut pending = vec![prop];
        let mut animated = 0;
        while let Some(entity) = pending.pop() {
            animated += usize::from(app.world().get::<AnimatedSceneHandle>(entity).is_some());
            if let Some(children) = app.world().get::<Children>(entity) { pending.extend(children.iter()); }
        }
        assert_eq!(animated, 1, "Actor must install the generated object animation scene");
    }

    pub(crate) fn loaded_bow_scene() -> (App, Entity, Entity) {
        use bevy_animation_graph::core::{animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance},
            animation_graph_player::AnimationGraphPlayer};
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        let scene = app.world().resource::<AssetServer>().load("ALS/OverlayObjects/Bow/B_Als_Bow.animscn.ron");
        let object = app.world_mut().spawn(AnimatedSceneHandle::new(scene.clone())).id();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let player = loop {
            app.update();
            if let bevy::asset::LoadState::Failed(error) = app.world().resource::<AssetServer>().load_state(scene.id()) {
                panic!("Bow scene failed: {error}");
            }
            if let Some(instance) = app.world().get::<AnimatedSceneInstance>(object) {
                break instance.player_entity();
            }
            assert!(std::time::Instant::now() < deadline, "Bow scene load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap().set_global_input_data("draw_amount", 0.0);
        app.update();
        (app, object, player)
    }

    #[test]
    fn bow_object_scene_loads_and_samples_the_generated_draw_graph() {
        use bevy_animation_graph::core::animation_clip::GraphClip;
        let (mut app, _, player) = loaded_bow_scene();
        let clip = app.world().resource::<AssetServer>().load::<GraphClip>("ALS/OverlayObjects/Bow/A_Als_Bow_Draw.anim.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !app.world().resource::<Assets<GraphClip>>().contains(&clip) {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Bow source clip load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mut string_positions = Vec::new();
        for amount in [0.0, 0.5, 1.0] {
            app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap().set_global_input_data("draw_amount", amount);
            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert!(player.get_error().is_none(), "{:?}", player.get_error());
            let pose = player.get_default_output_pose().expect("Bow must output its evaluated pose");
            let expected = crate::extensions::source_sequence_player::sample_pose(app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap(), amount);
            assert_eq!(pose.bones.len(), 24);
            for (id, index) in &expected.paths {
                let actual = pose.get_bone(*id).unwrap();
                let expected = &expected.bones[*index];
                assert!(actual.translation.unwrap().abs_diff_eq(expected.translation.unwrap(), 1e-6));
                assert!(actual.rotation.unwrap().abs_diff_eq(expected.rotation.unwrap(), 1e-6));
            }
            let skeleton = app.world().resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>().get(&pose.skeleton).unwrap();
            let string = skeleton.iter_paths().find(|path| path.parts.last().is_some_and(|name| name.as_str() == "string_mid")).unwrap();
            string_positions.push(pose.get_bone(string.id()).unwrap().translation.unwrap());
        }
        assert!(string_positions[0].distance(string_positions[2]) > 0.2);
        assert!(string_positions[1].distance(string_positions[0]) > 0.05);
    }

    #[test]
    fn bow_actor_uses_current_owner_curve_and_preserves_component_lifecycle() {
        use bevy_animation_graph::core::{animation_graph::AnimationGraph, animation_node::AnimationNode, edge_data::DataSpec};
        let (mut app, avatar, owner_player) = loaded_bow_scene();
        OverlayObjectPlugin.build(&mut app);
        crate::extensions::overlay_materials::OverlayMaterialsPlugin.build(&mut app);
        app.world_mut().spawn((Name::new(LEFT_GUN_BONE), Transform::IDENTITY, ChildOf(avatar)));
        let socket = app.world_mut().spawn((Name::new("hand_l"), Transform::IDENTITY, ChildOf(avatar))).id();
        let character = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY, OverlayState::Bow,
            AlsCharacterAssembly { avatar, camera: None })).id();
        app.world_mut().entity_mut(avatar).insert(ChildOf(character));
        let clip = app.world().resource::<AssetServer>().load("ALS/OverlayObjects/Bow/A_Als_Bow_Draw.anim.ron");
        let mut graph = AnimationGraph::new();
        let node = AnimationNode::new("Owner aiming curve", crate::extensions::SourceSequenceEvaluatorNode {
            clip, explicit_value_key: "owner_aiming".into(),
            curves: crate::extensions::AlsPoseCurves(std::collections::HashMap::from([("PoseAiming".into(), crate::extensions::AlsFloatCurve {
                times: vec![0.0, 1.0], values: vec![0.0, 1.0], ..default()
            })])), ..default()
        });
        graph.add_output_data("pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        graph.add_node(node);
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        app.world_mut().get_mut::<AnimationGraphPlayer>(owner_player).unwrap().start(graph);
        app.world_mut().get_mut::<AnimationGraphPlayer>(owner_player).unwrap().set_global_input_data("owner_aiming", 0.0);
        app.world_mut().entity_mut(owner_player).insert(crate::extensions::post_physics_animation::PostPhysicsAnimation);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let (prop, object_player) = loop {
            app.update();
            if let Some(prop) = app.world().get::<AlsOverlayObjectState>(character).and_then(|state| state.prop)
                && let Some(object) = app.world().get::<AlsOverlayObject>(prop)
                && let Some(scene) = app.world().get::<AnimatedSceneInstance>(object.content)
                && app.world().get::<AvatarMaterialOverride>(prop).is_some()
                && app.world().get::<AnimationGraphPlayer>(scene.player_entity()).is_some_and(|player| player.get_default_output_pose().is_some()) {
                break (prop, scene.player_entity());
            }
            assert!(std::time::Instant::now() < deadline, "Bow actor assembly timed out: state={:?}, object={:?}, scene={}, material={:?}",
                app.world().get::<AlsOverlayObjectState>(character),
                app.world().get::<AlsOverlayObjectState>(character).and_then(|state| state.prop).and_then(|entity| app.world().get::<AlsOverlayObject>(entity)),
                app.world().get::<AlsOverlayObjectState>(character).and_then(|state| state.prop).and_then(|entity| app.world().get::<AlsOverlayObject>(entity)).is_some_and(|object| app.world().get::<AnimatedSceneInstance>(object.content).is_some()),
                app.world().get::<AlsOverlayObjectState>(character).and_then(|state| state.prop).and_then(|entity| app.world().get::<AvatarMaterialApplied>(entity)));
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let components = app.world().get::<AlsOverlayObjectState>(character).unwrap().components;
        assert_eq!(prop, components[1]);
        let material_handles = &app.world().get::<AvatarMaterialOverride>(prop).unwrap().0;
        assert_eq!(material_handles.len(), 3);
        let definition = SOURCE_LIBRARY.0[&OverlayState::Bow].object.as_ref().unwrap();
        for (handle, source) in material_handles.iter().zip(&definition.materials) {
            let material = app.world().resource::<Assets<StandardMaterial>>().get(handle).unwrap();
            assert_eq!(material.base_color.to_linear().to_f32_array(), source.linear_rgba);
            assert_eq!(material.perceptual_roughness, source.roughness);
            assert_eq!(material.reflectance, source.reflectance);
        }
        for amount in [0.0, 0.75, 0.25, 1.0] {
            let before = app.world().get::<AnimationGraphPlayer>(object_player).unwrap().get_context_arena().unwrap().frame_index();
            app.world_mut().get_mut::<AnimationGraphPlayer>(owner_player).unwrap().set_global_input_data("owner_aiming", amount);
            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(object_player).unwrap();
            assert!(player.get_error().is_none(), "{:?}", player.get_error());
            assert_eq!(player.get_context_arena().unwrap().frame_index(), before + 1);
            assert_eq!(player.get_global_input_data("draw_amount").unwrap().as_f32().unwrap(), amount);
            let pose = player.get_default_output_pose().unwrap();
            assert_eq!(pose.timestamp, amount);
            let skeleton = app.world().resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>().get(&pose.skeleton).unwrap();
            let string = skeleton.iter_paths().find(|path| path.parts.last().is_some_and(|name| name.as_str() == "string_mid")).unwrap();
            let bone = player.bone_entity(string.id()).unwrap();
            assert!(app.world().get::<Transform>(bone).unwrap().translation.abs_diff_eq(pose.get_bone(string.id()).unwrap().translation.unwrap(), 1e-6));
        }
        app.world_mut().get_mut::<AlsOverlayObjectConfig>(character).unwrap().use_gun_bones = false;
        app.update();
        assert_eq!(app.world().get::<ChildOf>(prop).unwrap().parent(), socket);
        assert_eq!(app.world().get::<Transform>(prop).unwrap().translation, definition.socket_translation);
        assert_eq!(app.world().get::<Transform>(prop).unwrap().rotation, definition.socket_rotation);
        for component in components {
            assert_eq!(app.world().get::<ChildOf>(component).unwrap().parent(), socket);
            assert_eq!(app.world().get::<Transform>(component), app.world().get::<Transform>(prop));
        }
        assert_eq!(app.world().get::<AnimatedSceneInstance>(app.world().get::<AlsOverlayObject>(prop).unwrap().content).unwrap().player_entity(), object_player);
        for (ragdoll, mantle, kind, visible) in [(true, false, MantleKind::Low, false), (false, false, MantleKind::Low, true),
            (false, true, MantleKind::High, false), (false, true, MantleKind::Low, true), (false, false, MantleKind::Low, true)] {
            app.world_mut().entity_mut(character).insert((if ragdoll { AlsNotifyLocomotionAction::Ragdolling } else { AlsNotifyLocomotionAction::None },
                crate::als::mantle::AlsMantleState { active: mantle, kind, ..default() }));
            app.update();
            let state = app.world().get::<AlsOverlayObjectState>(character).unwrap();
            assert_eq!(state.components, components);
            assert_eq!(state.prop, visible.then_some(prop));
            assert_eq!(app.world().get::<AlsOverlayObject>(prop).is_some(), visible);
            if ragdoll { assert!(app.world().get_entity(object_player).is_err()); }
        }
        app.world_mut().entity_mut(character).despawn();
        app.update();
        assert!(components.iter().all(|entity| app.world().get_entity(*entity).is_err()));
    }

    #[test]
    fn barrel_actor_consumes_source_material_and_complete_socket() {
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        OverlayObjectPlugin.build(&mut app);
        let avatar = app.world_mut().spawn(Transform::IDENTITY).id();
        let hand = app.world_mut().spawn((Name::new("hand_l"), Transform::IDENTITY, ChildOf(avatar))).id();
        let character = app.world_mut().spawn((AlsCharacter, OverlayState::Barrel, AlsOverlayObjectConfig { use_gun_bones: false },
            AlsCharacterAssembly { avatar, camera: None })).id();
        app.update();
        app.update();
        let prop = app.world().get::<AlsOverlayObjectState>(character).unwrap().prop.unwrap();
        assert_eq!(app.world().get::<ChildOf>(prop).unwrap().parent(), hand);
        let transform = app.world().get::<Transform>(prop).unwrap();
        assert!(transform.translation.abs_diff_eq(Vec3::new(0.11606541, 0.17824009, -0.24229429), 1e-6));
        assert!(!transform.rotation.abs_diff_eq(Quat::IDENTITY, 0.01));
        let handles = &app.world().get::<AvatarMaterialOverride>(prop).expect("Source object material must be installed").0;
        assert_eq!(handles.len(), 1);
        let material = app.world().resource::<Assets<StandardMaterial>>().get(&handles[0]).unwrap();
        assert_eq!(material.base_color.to_linear().to_f32_array(), [0.93, 0.29, 1.0, 1.0]);
        assert_eq!(material.perceptual_roughness, 1.0);
    }

    #[test]
    fn pistol_mode_switch_keeps_the_same_mesh_content() {
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        OverlayObjectPlugin.build(&mut app);
        let avatar = app.world_mut().spawn(Transform::IDENTITY).id();
        app.world_mut().spawn((Name::new(RIGHT_GUN_BONE), Transform::IDENTITY, ChildOf(avatar)));
        app.world_mut().spawn((Name::new(LEFT_GUN_BONE), Transform::IDENTITY, ChildOf(avatar)));
        let character = app.world_mut().spawn((AlsCharacter, OverlayState::PistolOneHanded, AlsCharacterAssembly { avatar, camera: None })).id();
        app.update();
        app.update();
        let prop = app.world().get::<AlsOverlayObjectState>(character).unwrap().prop.unwrap();
        let content = app.world().get::<AlsOverlayObject>(prop).unwrap().content;
        *app.world_mut().get_mut::<OverlayState>(character).unwrap() = OverlayState::PistolTwoHanded;
        app.update();
        let object = app.world().get::<AlsOverlayObject>(prop).unwrap();
        assert_eq!(object.overlay, OverlayState::PistolTwoHanded);
        assert_eq!(object.content, content);
        assert!(app.world().get_entity(content).is_ok());
        let handle = app.world().resource::<OverlayLibraryHandle>().0.clone();
        {
            let mut libraries = app.world_mut().resource_mut::<Assets<OverlayLibrary>>();
            let mut library = libraries.get_mut(&handle).unwrap();
            library.0.get_mut(&OverlayState::PistolTwoHanded).unwrap().object.as_mut().unwrap().materials[0].roughness = 0.25;
        }
        app.update();
        assert_eq!(app.world().get::<AlsOverlayObject>(prop).unwrap().content, content);
        let material = &app.world().get::<AvatarMaterialOverride>(prop).unwrap().0[0];
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().get(material).unwrap().perceptual_roughness, 0.25);
        *app.world_mut().get_mut::<OverlayState>(character).unwrap() = OverlayState::Rifle;
        app.update();
        assert_ne!(app.world().get::<AlsOverlayObject>(prop).unwrap().content, content);
        assert!(app.world().get_entity(content).is_err());
        let rifle_content = app.world().get::<AlsOverlayObject>(prop).unwrap().content;
        let handle = app.world().resource::<OverlayLibraryHandle>().0.clone();
        {
            let mut libraries = app.world_mut().resource_mut::<Assets<OverlayLibrary>>();
            let mut library = libraries.get_mut(&handle).unwrap();
            let torch = library.0[&OverlayState::Torch].object.clone();
            library.0.get_mut(&OverlayState::Rifle).unwrap().object = torch;
        }
        app.update();
        let prop = app.world().get::<AlsOverlayObjectState>(character).unwrap().prop.unwrap();
        assert_eq!(app.world().get::<AlsOverlayObject>(prop).unwrap().definition.kind, OverlayObjectKind::Static);
        assert!(app.world().get_entity(rifle_content).is_err());
    }

    #[test]
    fn source_overlay_specs_keep_asset_kind_and_gun_side() {
        assert!(overlay_object_spec(&SOURCE_LIBRARY, OverlayState::Default).is_none());
        let rifle = overlay_object_spec(&SOURCE_LIBRARY, OverlayState::Rifle).unwrap();
        assert_eq!(rifle.kind, OverlayObjectKind::Skeletal);
        assert_eq!(rifle.socket_name, "Rifle");
        assert!(!rifle.use_left_gun_bone);
        let torch = overlay_object_spec(&SOURCE_LIBRARY, OverlayState::Torch).unwrap();
        assert_eq!(torch.kind, OverlayObjectKind::Static);
        assert_eq!(torch.socket_name, "Torch");
        assert!(torch.use_left_gun_bone);
        assert_eq!(
            overlay_object_spec(&SOURCE_LIBRARY, OverlayState::PistolTwoHanded),
            overlay_object_spec(&SOURCE_LIBRARY, OverlayState::PistolOneHanded)
        );
    }

    #[test]
    fn gun_bone_mode_uses_source_virtual_bones() {
        let rifle = overlay_object_spec(&SOURCE_LIBRARY, OverlayState::Rifle).unwrap();
        let torch = overlay_object_spec(&SOURCE_LIBRARY, OverlayState::Torch).unwrap();
        assert_eq!(attachment_bone(rifle, true), RIGHT_GUN_BONE);
        assert_eq!(attachment_bone(torch, true), LEFT_GUN_BONE);
        assert_eq!(attachment_bone(rifle, false), "hand_r");
        assert_eq!(attachment_bone(torch, false), "hand_l");
    }

    #[test]
    fn source_lifecycle_clears_high_mantle_and_ragdoll_but_keeps_low_mantle() {
        assert!(overlay_object_visible(&SOURCE_LIBRARY,
            OverlayState::Rifle,
            false,
            false,
            None
        ));
        assert!(!overlay_object_visible(&SOURCE_LIBRARY,
            OverlayState::Rifle,
            false,
            true,
            Some(MantleKind::High)
        ));
        assert!(overlay_object_visible(&SOURCE_LIBRARY,
            OverlayState::Rifle,
            false,
            true,
            Some(MantleKind::LowLeft)
        ));
        assert!(!overlay_object_visible(&SOURCE_LIBRARY,
            OverlayState::Rifle,
            true,
            false,
            None
        ));
    }
}
