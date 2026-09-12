use als_core::character::{
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, set_collision_layer_owner,
};
use als_core::mantle::{AlsMantleState, AlsMantleTargetBase, MantleKind};
use avian3d::prelude::{CollisionLayers, LinearVelocity, Rotation};
use bevy::prelude::*;
use bevy_animation_graph::core::animation_clip::GraphClip;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::skeleton::Skeleton;

use crate::root_motion_link::mesh_to_entity_yaw;

#[derive(Resource)]
struct MantleAssets([Handle<GraphClip>; 3]);

impl MantleAssets {
    fn clip(&self, kind: MantleKind) -> &Handle<GraphClip> {
        &self.0[if kind.is_high() {
            0
        } else if kind == MantleKind::LowLeft {
            1
        } else {
            2
        }]
    }
}

fn load_mantle_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(MantleAssets([
        assets.load("animations/Actions/Mantle/A_Als_Mantle_High.anim.ron"),
        assets.load("animations/Actions/Mantle/A_Als_Mantle_Low_Left.anim.ron"),
        assets.load("animations/Actions/Mantle/A_Als_Mantle_Low_Right.anim.ron"),
    ]));
}

fn sample_mantle_root(clip: &GraphClip, skeleton: &Skeleton, time: f32) -> Option<Transform> {
    let pose = als_ext::source_sequence_player::sample_pose(clip, time);
    let root = als_ext::root_motion_defaults::derive_default_root_bone(clip, skeleton)?.id();
    let bone = pose.get_bone(root)?;
    let reference = skeleton.default_transforms(root)?.local;
    let basis = mesh_to_entity_yaw();
    Some(Transform {
        translation: basis * bone.translation.unwrap_or(reference.translation),
        rotation: (basis * bone.rotation.unwrap_or(reference.rotation) * basis.conjugate())
            .normalize(),
        ..default()
    })
}

pub struct AlsMantleDrivePlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsMantleDriveSet;

impl Plugin for AlsMantleDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_mantle_assets).add_systems(
            Update,
            (
                ensure_mantle_state,
                mantle_pose_system.after(ensure_mantle_state),
            ),
        );
        app.configure_sets(
            PostUpdate,
            AlsMantleDriveSet.after(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            mantle_drive_system
                .after(AnimationGraphSet::PrePhysics)
                .in_set(AlsMantleDriveSet),
        );
    }
}

fn ensure_mantle_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsMantleState>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsMantleState::default());
    }
}

fn mantle_pose_system(
    mut commands: Commands,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, mut m, mut vel, layers, mut ownership) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut ownership,
            layers.copied(),
            AlsCollisionLayerOwner::Mantle,
            m.active,
        );
    }
}

#[allow(clippy::type_complexity)]
fn mantle_drive_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            &mut Transform,
            Option<&mut Rotation>,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
) {
    let dt = time.delta_secs();
    for (entity, mut m, mut vel, mut transform, rotation, layers, mut ownership) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
        if !target_valid || !m.active || dt <= 1e-8 {
            set_collision_layer_owner(
                &mut commands,
                entity,
                &mut ownership,
                layers.copied(),
                AlsCollisionLayerOwner::Mantle,
                false,
            );
            continue;
        }

        let Some(clip) = clips.get(assets.clip(m.kind)) else {
            continue;
        };
        let Some(skeleton) = skeletons.get(&clip.skeleton) else {
            continue;
        };
        if !m.root_motion_ready {
            let (Some(start_root), Some(end_root)) = (
                sample_mantle_root(clip, skeleton, m.start_offset),
                sample_mantle_root(clip, skeleton, clip.duration),
            ) else {
                continue;
            };
            m.prepare_root_motion(start_root, end_root);
        }
        let previous_time = m.time;
        let current_time = (previous_time + dt).min(m.action_end());
        let Some(root) = sample_mantle_root(clip, skeleton, m.clip_time_at(current_time)) else {
            continue;
        };
        let target_actor_transform = m.target_actor_transform_at(current_time, root);
        *vel = LinearVelocity((target_actor_transform.translation - transform.translation) / dt);
        transform.rotation = target_actor_transform.rotation;
        if let Some(mut rotation) = rotation {
            rotation.0 = target_actor_transform.rotation;
        }
        m.time = current_time;
        let finishing = current_time >= m.action_end();
        if finishing {
            m.active = false;
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut ownership,
            layers.copied(),
            AlsCollisionLayerOwner::Mantle,
            m.active,
        );
    }
}

fn mantle_target_base(
    state: &AlsMantleState,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<AlsMantleTargetBase> {
    let base = state.target_base?;
    let transform = transforms.get(base.entity).ok()?;
    Some(AlsMantleTargetBase {
        entity: base.entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
        scale: transform.scale(),
        relative_location: base.relative_location,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy_animation_graph::core::{
        animation_clip::loader::GraphClipLoader, skeleton::loader::SkeletonLoader,
    };

    #[test]
    fn mantle_absolute_root_drives_actual_clips_to_source_endpoints() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<GraphClip>()
        .init_asset_loader::<GraphClipLoader>()
        .init_asset::<Skeleton>()
        .init_asset_loader::<SkeletonLoader>()
        .add_systems(Startup, load_mantle_assets);
        app.finish();
        app.cleanup();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut previous = 0;
        loop {
            app.update();
            let assets = app.world().resource::<MantleAssets>();
            let clips = app.world().resource::<Assets<GraphClip>>();
            let skeletons = app.world().resource::<Assets<Skeleton>>();
            let loaded = assets
                .0
                .iter()
                .filter(|handle| {
                    clips
                        .get(*handle)
                        .and_then(|clip| skeletons.get(&clip.skeleton))
                        .is_some()
                })
                .count();
            if loaded != previous {
                eprintln!("mantle clips loaded {loaded}/3");
                previous = loaded;
            }
            if loaded == 3 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "mantle assets did not load within 30 seconds: {loaded}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        for kind in [
            MantleKind::High,
            MantleKind::InAir,
            MantleKind::LowLeft,
            MantleKind::LowRight,
        ] {
            let start = Vec3::new(-1.0, 0.9, 0.3);
            let target = Vec3::new(2.0, 2.0, -2.0);
            let mut mantle = AlsMantleState::default();
            mantle.begin(
                kind,
                start,
                target,
                if kind.is_high() { 1.5 } else { 0.75 },
                -0.6,
                0.8,
            );
            let duration = mantle.duration;
            {
                let assets = app.world().resource::<MantleAssets>();
                let clips = app.world().resource::<Assets<GraphClip>>();
                let clip = clips.get(assets.clip(kind)).unwrap();
                let skeletons = app.world().resource::<Assets<Skeleton>>();
                let skeleton = skeletons.get(&clip.skeleton).unwrap();
                let root = sample_mantle_root(clip, skeleton, clip.duration).unwrap();
                assert!(
                    root.translation.distance(Vec3::new(
                        0.0,
                        if kind.is_high() { 2.0 } else { 1.0 },
                        -0.65
                    )) < 1e-5
                );
                let initial = sample_mantle_root(clip, skeleton, mantle.start_offset).unwrap();
                assert!(mantle.start_offset > 0.0);
                let mut prepared = mantle;
                prepared.prepare_root_motion(initial, root);
                assert!(
                    prepared
                        .target_actor_transform_at(0.0, initial)
                        .translation
                        .distance(start)
                        < 1e-5
                );
                let raw = Transform::from_translation(
                    mesh_to_entity_yaw().conjugate() * initial.translation,
                );
                let source_inverse_rotation = mesh_to_entity_yaw() * raw.rotation.conjugate();
                let source_warp_rotation = Quat::from_rotation_y(-0.6) * source_inverse_rotation;
                let source_warp_location = start - source_warp_rotation * raw.translation;
                assert!(
                    prepared
                        .warp_start
                        .translation
                        .distance(source_warp_location)
                        < 1e-5
                );
            }
            let entity = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    mantle,
                    LinearVelocity::ZERO,
                    Transform::from_translation(start).with_rotation(Quat::from_rotation_y(-0.6)),
                    Rotation(Quat::from_rotation_y(-0.6)),
                    AlsCollisionLayerOwnership::default(),
                ))
                .id();
            let delta = 1.0 / 60.0;
            for _ in 0..=(duration / delta).ceil() as usize {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(delta));
                app.world_mut()
                    .run_system_once(mantle_drive_system)
                    .unwrap();
                let velocity = app.world().get::<LinearVelocity>(entity).unwrap().0;
                app.world_mut()
                    .get_mut::<Transform>(entity)
                    .unwrap()
                    .translation += velocity * delta;
                if !app.world().get::<AlsMantleState>(entity).unwrap().active {
                    break;
                }
            }
            let state = app.world().get::<AlsMantleState>(entity).unwrap();
            assert!(!state.active && state.root_motion_ready, "{kind:?}");
            let actor = app.world().get::<Transform>(entity).unwrap();
            assert!(
                actor.translation.distance(target) < 1e-4,
                "{kind:?}: {:?}",
                actor.translation
            );
            assert!(actor.rotation.angle_between(Quat::from_rotation_y(0.8)) < 1e-3);
            assert_eq!(
                app.world().get::<Rotation>(entity).unwrap().0,
                actor.rotation
            );
            app.world_mut().despawn(entity);
        }
    }
}
