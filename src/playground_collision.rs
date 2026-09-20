use als_core::character::AlsCollisionLayer;
use std::collections::HashMap;

use als_ext::{
    PlaygroundBoxCollider, PlaygroundColliderKind, PlaygroundCollisionSource,
    PlaygroundKinematic, PlaygroundKinematicKind, PlaygroundMovingCurve, PlaygroundMovingObject,
    PlaygroundPhysicsDriven, playground_moving_object_system,
};
use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PlaygroundCollisionPlugin;

impl Plugin for PlaygroundCollisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaygroundMeshColliderCache>()
            .init_resource::<PlaygroundMovingCurve>()
            .add_systems(
                Update,
                (
                    playground_moving_box_physics_bridge,
                    playground_mesh_physics_bridge,
                    playground_kinematic_velocity_bridge
                        .after(playground_moving_object_system)
                        .before(bevy_tnua::TnuaPipelineSystems::Sensors),
                ),
            )
            .add_systems(PostUpdate, playground_kinematic_velocity_bridge
                .after(PhysicsSystems::Prepare).before(PhysicsSystems::StepSimulation));
    }
}

#[derive(Component, Debug, Clone)]
pub struct PlaygroundMeshColliderBuilt {
    pub mesh_base: String,
    pub kind: PlaygroundColliderKind,
    pub vertices: usize,
    pub triangles: usize,
    pub surface_probes: Vec<Vec3>,
}

#[derive(Clone)]
struct PlaygroundMeshColliderTemplate {
    collider: Collider,
    vertices: usize,
    triangles: usize,
    surface_probes: Vec<Vec3>,
}

#[derive(Resource, Default)]
struct PlaygroundMeshColliderCache(HashMap<String, PlaygroundMeshColliderTemplate>);

struct PlaygroundMeshGeometry {
    vertices: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
}

pub fn playground_kinematic_velocity_bridge(
    time: Res<Time<Virtual>>,
    curve: Res<PlaygroundMovingCurve>,
    transforms: bevy::transform::helper::TransformHelper,
    mut platforms: Query<
        (&PlaygroundKinematic, &Position, &mut Rotation, Option<&ChildOf>, &mut LinearVelocity, &mut AngularVelocity),
        With<PlaygroundMovingObject>,
    >,
) {
    let delta = time.delta()
        .min(crate::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA)
        .as_secs_f32();
    for (kinematic, position, mut rotation, parent, mut linear, mut angular) in &mut platforms {
        match kinematic.kind {
            PlaygroundKinematicKind::MoveSpline { duration, start, end } => {
                let target = als_ext::playground::moving_object_position(
                    &curve, start, end, time.elapsed_secs(), duration,
                );
                linear.0 = if delta > 0.0 {
                    (target - position.0) / delta
                } else {
                    Vec3::ZERO
                };
                angular.0 = Vec3::ZERO;
            }
            PlaygroundKinematicKind::RotateWorldTime { rate_yzx_radians } => {
                linear.0 = Vec3::ZERO;
                let parent_rotation = parent.and_then(|parent| transforms.compute_global_transform(parent.parent()).ok())
                    .map_or(kinematic.base_transform.rotation, |parent| parent.rotation());
                let target = parent_rotation * als_ext::playground::rotating_object_rotation(rate_yzx_radians, time.elapsed_secs());
                let mut difference = (target * rotation.0.conjugate()).normalize();
                if difference.w < 0.0 { difference = -difference; }
                if delta > 0.0 {
                    angular.0 = difference.to_scaled_axis() / delta;
                } else {
                    angular.0 = Vec3::ZERO;
                    rotation.0 = target;
                }
            }
        }
    }
}

fn playground_moving_box_physics_bridge(
    mut commands: Commands,
    bodies: Query<(Entity, &PlaygroundBoxCollider, Option<&ChildOf>), Added<PlaygroundBoxCollider>>,
    moving_roots: Query<(), With<PlaygroundMovingObject>>,
) {
    for (entity, collider, parent) in &bodies {
        let Some(&ChildOf(root)) = parent else {
            continue;
        };
        if !moving_roots.contains(root) {
            continue;
        }
        commands.entity(entity).insert((Collider::cuboid(
            collider.half_extents.x * 2.0,
            collider.half_extents.y * 2.0,
            collider.half_extents.z * 2.0,
        ), CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL)));
        commands
            .entity(root)
            .insert((RigidBody::Kinematic, PlaygroundPhysicsDriven));
    }
}

fn playground_mesh_physics_bridge(
    mut commands: Commands,
    mut cache: ResMut<PlaygroundMeshColliderCache>,
    roots: Query<
        (Entity, &PlaygroundCollisionSource, &GlobalTransform),
        Without<PlaygroundMeshColliderBuilt>,
    >,
    scene_children: Query<&Children>,
    mesh_parts: Query<(&Mesh3d, &GlobalTransform)>,
    meshes: Res<Assets<Mesh>>,
) {
    for (root, source, root_transform) in &roots {
        if source.kind.is_box() {
            continue;
        }
        let template = if let Some(template) = cache.0.get(&source.mesh_base) {
            template.clone()
        } else {
            let Some(template) = build_mesh_collider_template(
                source,
                root_transform.to_matrix().inverse(),
                root,
                &scene_children,
                &mesh_parts,
                &meshes,
            ) else {
                continue;
            };
            cache.0.insert(source.mesh_base.clone(), template.clone());
            template
        };
        info!(
            "[PGCOL] built source={} kind={:?} verts={} triangles={}",
            source.mesh_base,
            template_kind(source.kind),
            template.vertices,
            template.triangles
        );
        commands.entity(root).insert((
            RigidBody::Static,
            template.collider,
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
            PlaygroundMeshColliderBuilt {
                mesh_base: source.mesh_base.clone(),
                kind: source.kind,
                vertices: template.vertices,
                triangles: template.triangles,
                surface_probes: template.surface_probes,
            },
        ));
    }
}

fn template_kind(kind: PlaygroundColliderKind) -> &'static str {
    match kind {
        PlaygroundColliderKind::Box => "box",
        PlaygroundColliderKind::Convex { .. } => "convex",
        PlaygroundColliderKind::TriMesh => "trimesh",
    }
}

fn build_mesh_collider_template(
    source: &PlaygroundCollisionSource,
    root_inverse: Mat4,
    root: Entity,
    scene_children: &Query<&Children>,
    mesh_parts: &Query<(&Mesh3d, &GlobalTransform)>,
    meshes: &Assets<Mesh>,
) -> Option<PlaygroundMeshColliderTemplate> {
    let mut geometry = PlaygroundMeshGeometry {
        vertices: Vec::new(),
        triangles: Vec::new(),
    };
    let mut vertex_indices = HashMap::new();
    append_mesh_geometry(
        root,
        root_inverse,
        scene_children,
        mesh_parts,
        meshes,
        &mut geometry,
        &mut vertex_indices,
    );
    if geometry.vertices.is_empty() || geometry.triangles.is_empty() {
        return None;
    }
    let vertices = geometry.vertices.len();
    let triangles = geometry.triangles.len();
    let surface_probes = if source.mesh_base.starts_with("SM_Als_Stairs_") {
        select_stair_surface_probes(&geometry.vertices, &geometry.triangles)
    } else {
        Vec::new()
    };
    let collider = match source.kind {
        PlaygroundColliderKind::Box => return None,
        PlaygroundColliderKind::Convex { source_hulls } => {
            Collider::convex_decomposition_with_config(
                geometry.vertices,
                geometry.triangles,
                VhacdParameters {
                    max_convex_hulls: source_hulls as u32,
                    ..default()
                },
            )
        }
        PlaygroundColliderKind::TriMesh => {
            match Collider::try_trimesh(geometry.vertices, geometry.triangles) {
                Ok(collider) => collider,
                Err(error) => {
                    error!(
                        "[PGCOL] trimesh failed source={} error={error:?}",
                        source.mesh_base
                    );
                    return None;
                }
            }
        }
    };
    Some(PlaygroundMeshColliderTemplate {
        collider,
        vertices,
        triangles,
        surface_probes,
    })
}

fn append_mesh_geometry(
    entity: Entity,
    root_inverse: Mat4,
    scene_children: &Query<&Children>,
    mesh_parts: &Query<(&Mesh3d, &GlobalTransform)>,
    meshes: &Assets<Mesh>,
    geometry: &mut PlaygroundMeshGeometry,
    vertex_indices: &mut HashMap<[u32; 3], u32>,
) {
    if let Ok((mesh_handle, transform)) = mesh_parts.get(entity)
        && let Some(mesh) = meshes.get(&mesh_handle.0)
        && let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        && let Some(values) = positions.as_float3()
    {
        let relative = root_inverse * transform.to_matrix();
        let indices = match mesh.indices() {
            Some(bevy::render::mesh::Indices::U32(indices)) => indices.clone(),
            Some(bevy::render::mesh::Indices::U16(indices)) => {
                indices.iter().map(|index| *index as u32).collect()
            }
            None => (0..values.len() as u32).collect(),
        };
        for triangle in indices.chunks_exact(3) {
            let Some(first) = values.get(triangle[0] as usize) else {
                continue;
            };
            let Some(second) = values.get(triangle[1] as usize) else {
                continue;
            };
            let Some(third) = values.get(triangle[2] as usize) else {
                continue;
            };
            let triangle = [
                deduplicated_vertex(
                    relative.transform_point3(Vec3::from(*first)),
                    geometry,
                    vertex_indices,
                ),
                deduplicated_vertex(
                    relative.transform_point3(Vec3::from(*second)),
                    geometry,
                    vertex_indices,
                ),
                deduplicated_vertex(
                    relative.transform_point3(Vec3::from(*third)),
                    geometry,
                    vertex_indices,
                ),
            ];
            if triangle[0] != triangle[1]
                && triangle[1] != triangle[2]
                && triangle[2] != triangle[0]
            {
                geometry.triangles.push(triangle);
            }
        }
    }
    if let Ok(children) = scene_children.get(entity) {
        for child in children.iter() {
            append_mesh_geometry(
                child,
                root_inverse,
                scene_children,
                mesh_parts,
                meshes,
                geometry,
                vertex_indices,
            );
        }
    }
}

fn deduplicated_vertex(
    position: Vec3,
    geometry: &mut PlaygroundMeshGeometry,
    vertex_indices: &mut HashMap<[u32; 3], u32>,
) -> u32 {
    let key = [
        position.x.to_bits(),
        position.y.to_bits(),
        position.z.to_bits(),
    ];
    if let Some(index) = vertex_indices.get(&key) {
        return *index;
    }
    let index = geometry.vertices.len() as u32;
    geometry.vertices.push(position);
    vertex_indices.insert(key, index);
    index
}

fn select_stair_surface_probes(vertices: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut candidates = triangles
        .iter()
        .filter_map(|triangle| {
            let first = *vertices.get(triangle[0] as usize)?;
            let second = *vertices.get(triangle[1] as usize)?;
            let third = *vertices.get(triangle[2] as usize)?;
            let normal = (second - first).cross(third - first);
            (normal.y > 1e-4).then_some((first + second + third) / 3.0)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.y.total_cmp(&right.y));
    let Some(low) = candidates.first().copied() else {
        return Vec::new();
    };
    let Some(high) = candidates
        .iter()
        .rev()
        .copied()
        .find(|candidate| candidate.y - low.y > 0.2)
    else {
        return Vec::new();
    };
    vec![low, high]
}

#[cfg(test)]
mod tests {
    use super::*;
    use als_core::{character::{AlsCharacterBundle, AlsCharacterPlugin}, control_scheme::{AlsControlScheme, AlsControlSchemeConfig},
        gait::AlsDesiredGait, input::AlsInputCommand, motion::{AlsMotionPlugin, initial_als_control_scheme_config},
        params::AlsMovementParams, state::{Gait, LocomotionState}, transition::AlsStateTransitionPlugin};
    use bevy::{asset::AssetEvent, ecs::system::RunSystemOnce, time::TimeUpdateStrategy};

    #[test]
    fn rotating_actor_world_time_and_yaw_match_source() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, PhysicsPlugins::new(PostUpdate),
            PlaygroundCollisionPlugin, crate::source_physics_timing::SourcePhysicsTimingPlugin))
            .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.025)));
        app.finish();
        for _ in 0..40 { app.update(); }
        let transform = Transform::from_xyz(13.5, -0.5, -24.0)
            .with_rotation(Quat::from_rotation_y(0.3) * Quat::from_rotation_z(0.2))
            .with_scale(Vec3::new(1.5, 0.8, 2.0));
        let anchor = app.world_mut().spawn((transform, GlobalTransform::from(transform))).id();
        let mesh = app.world_mut().spawn((PlaygroundMovingObject, PlaygroundPhysicsDriven, RigidBody::Kinematic,
            PlaygroundKinematic { base_transform: transform, kind: PlaygroundKinematicKind::RotateWorldTime {
                rate_yzx_radians: bevy::math::DVec3::new(-100.0_f64.to_radians(), 0.0, 0.0) } },
            ChildOf(anchor), Transform::IDENTITY, GlobalTransform::from(transform))).id();
        for frame in 0..240 {
            let delta = [1.0/120.0, 1.0/60.0, 0.05, 0.013, 0.0][frame % 5];
            if frame == 50 {
                let mut rotation = app.world_mut().get_mut::<Rotation>(mesh).unwrap();
                rotation.0 = -rotation.0;
            }
            app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            app.update();
            let time = app.world().resource::<Time>().elapsed_secs();
            let expected = transform.rotation * Quat::from_rotation_y(-100.0_f32.to_radians() * time);
            let actual = app.world().get::<Rotation>(mesh).unwrap().0;
            let angular_step = app.world().get::<AngularVelocity>(mesh).unwrap().length()
                * delta.min(crate::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA.as_secs_f32());
            assert!(angular_step <= std::f32::consts::PI + 1e-4, "Rotation did not use the shortest quaternion arc");
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                assert!((actual * axis).abs_diff_eq(expected * axis, 1e-4),
                    "Rotating mesh differs from source world time: time={time} actual={actual:?} expected={expected:?}");
            }
            assert_eq!(app.world().get::<Transform>(anchor).unwrap().rotation, transform.rotation);
        }
    }

    #[test]
    fn moving_platform_reaches_the_source_position_after_late_start_and_variable_steps() {
        use als_ext::{PlaygroundKinematic, PlaygroundKinematicKind};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, PhysicsPlugins::new(PostUpdate),
            PlaygroundCollisionPlugin, crate::source_physics_timing::SourcePhysicsTimingPlugin))
            .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
            .insert_resource(als_ext::playground::load_moving_object_curve().unwrap())
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.025)))
            .add_systems(Update, playground_moving_object_system);
        app.finish();
        for _ in 0..40 { app.update(); }
        let start = Vec3::new(19.0, 1.0, -28.75);
        let end = start - Vec3::X * 6.5;
        let transform = Transform::from_translation(start);
        let anchor = app.world_mut().spawn((transform, GlobalTransform::from(transform))).id();
        let platform = app.world_mut().spawn((PlaygroundMovingObject, PlaygroundPhysicsDriven,
            PlaygroundKinematic { base_transform:transform,
                kind:PlaygroundKinematicKind::MoveSpline { duration:1.75, start, end } },
            RigidBody::Kinematic, ChildOf(anchor), Transform::IDENTITY, GlobalTransform::from(transform))).id();
        for frame in 0..240 {
            let delta = [1.0/120.0, 1.0/60.0, 0.05, 0.013, 0.0][frame % 5];
            *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
            app.update();
            let t = app.world().resource::<Time>().elapsed_secs();
            let expected = als_ext::playground::moving_object_position(app.world().resource::<als_ext::PlaygroundMovingCurve>(), start, end, t, 1.75);
            let actual = app.world().get::<Position>(platform).unwrap().0;
            assert!(actual.abs_diff_eq(expected, 1e-4), "Platform drifted at frame={frame}: actual={actual:?} expected={expected:?}");
            assert_eq!(app.world().get::<Transform>(anchor).unwrap().translation, start);
        }
    }

    #[test]
    fn imported_stair_descent_exits_initial_contact_without_entering_the_stair() {
        let data: serde_json::Value = serde_json::from_str(include_str!("../assets/models/playground/SM_Als_Stairs_03.gltf")).unwrap();
        let bytes = include_bytes!("../assets/models/playground/SM_Als_Stairs_03.bin");
        let primitive = &data["meshes"][0]["primitives"][0];
        let accessor_bytes = |index: usize, width: usize| {
            let accessor = &data["accessors"][index];
            let view = &data["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
            let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
            assert_eq!(view["byteStride"].as_u64().unwrap_or(width as u64) as usize, width);
            &bytes[offset..offset + accessor["count"].as_u64().unwrap() as usize * width]
        };
        let positions = accessor_bytes(primitive["attributes"]["POSITION"].as_u64().unwrap() as usize, 12)
            .chunks_exact(12).map(|point| std::array::from_fn::<_,3,_>(|i| f32::from_le_bytes(point[i*4..i*4+4].try_into().unwrap()))).collect::<Vec<_>>();
        let indices = accessor_bytes(primitive["indices"].as_u64().unwrap() as usize, 2).chunks_exact(2)
            .map(|index| u16::from_le_bytes(index.try_into().unwrap()) as u32).collect();
        let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            PhysicsPlugins::new(PostUpdate), bevy_tnua_avian3d::TnuaAvian3dPlugin::new(Update),
            bevy_tnua::TnuaControllerPlugin::<AlsControlScheme>::new(Update), AlsStateTransitionPlugin,
            AlsCharacterPlugin, AlsMotionPlugin, PlaygroundCollisionPlugin, crate::source_physics_timing::SourcePhysicsTimingPlugin))
            .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.0170989)));
        app.finish();
        let source = Transform::from_xyz(-30.0, 0.36, -30.0);
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        let root = app.world_mut().spawn((PlaygroundCollisionSource::new("SM_Als_Stairs_03"), source, GlobalTransform::from(source))).id();
        app.world_mut().spawn((ChildOf(root), Mesh3d(mesh), Transform::IDENTITY, GlobalTransform::from(source)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(80.0,0.2,80.0), Transform::from_xyz(0.0,-0.1,-30.0)));
        let barrier = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(6.67,3.0,0.2), Transform::from_xyz(-26.665,6.4,-34.5))).id();
        let params = AlsMovementParams::default();
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&params));
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params);
        let start = Vec3::new(-26.665, 6.321502, -35.25028);
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform.translation = start;
        let actor = app.world_mut().spawn(bundle).id();
        for _ in 0..30 { app.update(); }
        assert!(app.world().get::<PlaygroundMeshColliderBuilt>(root).is_some());
        let capsule = app.world().get::<Collider>(actor).unwrap().clone();
        app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            let filter = SpatialQueryFilter::from_excluded_entities([actor]);
            let config = ShapeCastConfig::from_max_distance(6.5*0.012236);
            let hit = spatial.cast_shape(&capsule, start, Quat::IDENTITY, Dir3::Z, &config, &filter).unwrap();
            assert_eq!(hit.distance, 0.0);
            assert!(hit.normal1.z > 0.99);
            assert!(((hit.point1 - hit.point2).dot(hit.normal1) - 0.000278).abs() < 1e-5);
            let outward = ShapeCastConfig { ignore_origin_penetration:true, ..config };
            assert!(spatial.cast_shape(&capsule, start, Quat::IDENTITY, Dir3::Z, &outward, &filter).is_none());
            assert!(spatial.cast_shape(&capsule, start, Quat::IDENTITY, Dir3::NEG_Z, &outward, &filter).is_some());
            let onward = ShapeCastConfig { max_distance:1.0, ..outward };
            let hit = spatial.cast_shape(&capsule, start, Quat::IDENTITY, Dir3::Z, &onward, &filter).unwrap();
            assert_eq!(hit.entity, barrier);
            assert!(hit.distance > 0.3 && hit.distance < 0.4);
        }).unwrap();
        app.world_mut().get_mut::<Transform>(actor).unwrap().translation = start;
        app.world_mut().get_mut::<Position>(actor).unwrap().0 = start;
        app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::Z * 6.5;
        *app.world_mut().get_mut::<LocomotionState>(actor).unwrap() = LocomotionState::Grounded;
        app.world_mut().get_mut::<AlsDesiredGait>(actor).unwrap().0 = Gait::Sprinting;
        app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().move_dir = Vec2::Y;
        app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().sprint_held = true;
        app.world_mut().get_mut::<als_core::motion::AlsViewYaw>(actor).unwrap().0 = std::f32::consts::PI;
        *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.012236));
        app.update();
        let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
        let position = app.world().get::<Position>(actor).unwrap().0;
        assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
        assert!((velocity.z - 6.5).abs() < 0.1 && position.z - start.z > 0.075,
            "Recorded overlap stopped motion: position={position:?} velocity={velocity:?}");
    }
}
