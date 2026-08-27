use std::collections::HashMap;

use als_core::character::AlsCrushRecoveryFloor;
use als_ext::{
    PlaygroundBoxCollider, PlaygroundColliderKind, PlaygroundCollisionSource,
    PlaygroundMovingObject, PlaygroundPhysicsDriven,
};
use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PlaygroundCollisionPlugin;

impl Plugin for PlaygroundCollisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaygroundMeshColliderCache>()
            .add_systems(
                Update,
                (
                    playground_box_physics_bridge,
                    playground_mesh_physics_bridge,
                ),
            );
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

fn playground_box_physics_bridge(
    mut commands: Commands,
    bodies: Query<
        (Entity, &Name, &PlaygroundBoxCollider, Option<&ChildOf>),
        Added<PlaygroundBoxCollider>,
    >,
    moving_roots: Query<(), With<PlaygroundMovingObject>>,
    root_names: Query<&Name>,
) {
    for (entity, name, collider, parent) in &bodies {
        let shape = Collider::cuboid(
            collider.half_extents.x * 2.0,
            collider.half_extents.y * 2.0,
            collider.half_extents.z * 2.0,
        );
        if let Some(&ChildOf(root)) = parent
            && moving_roots.contains(root)
        {
            commands.entity(entity).insert(shape);
            commands
                .entity(root)
                .insert((RigidBody::Kinematic, PlaygroundPhysicsDriven));
        } else {
            let parent_is_floor = parent.is_some_and(|&ChildOf(root)| {
                root_names
                    .get(root)
                    .is_ok_and(|root_name| root_name.as_str().ends_with(":BP_SimpleObject5"))
            });
            let is_floor = parent_is_floor || name.as_str() == "BP_SimpleObject5:aabb";
            let mut entity_commands = commands.entity(entity);
            entity_commands.insert((RigidBody::Static, shape));
            if is_floor {
                entity_commands.insert(AlsCrushRecoveryFloor {
                    half_extents: collider.half_extents,
                });
            }
        }
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
