use bevy::prelude::*;

pub const AVATAR_MAIN_MATERIAL_SLOT: usize = 10;

pub struct OverlayMaterialsPlugin;

impl Plugin for OverlayMaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverlayMaterialLibrary>()
            .add_systems(PreStartup, load_overlay_materials)
            .add_systems(
                Update,
                (avatar_material_bridge, overlay_prop_material_bridge),
            );
    }
}

#[derive(Resource, Default, Clone)]
pub struct OverlayMaterialLibrary {
    pub character_body: Handle<StandardMaterial>,
    pub pistol: Handle<StandardMaterial>,
    pub rifle: Handle<StandardMaterial>,
    pub bow_01: Handle<StandardMaterial>,
    pub bow_02: Handle<StandardMaterial>,
    pub bow_03: Handle<StandardMaterial>,
    pub barrel: Handle<StandardMaterial>,
    pub box_green: Handle<StandardMaterial>,
    pub binoculars: Handle<StandardMaterial>,
    pub torch_01: Handle<StandardMaterial>,
    pub torch_02: Handle<StandardMaterial>,
    pub torch_03: Handle<StandardMaterial>,
}

fn solid_mat(
    materials: &mut Assets<StandardMaterial>,
    color: [f32; 3],
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(color[0], color[1], color[2]),
        perceptual_roughness: 0.7,
        ..default()
    })
}

fn load_overlay_materials(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let lib = OverlayMaterialLibrary {
        character_body: solid_mat(&mut materials, [1.0, 1.0, 1.0]),
        pistol: solid_mat(&mut materials, [1.0, 0.68, 0.05]),
        rifle: solid_mat(&mut materials, [1.0, 0.68, 0.05]),
        bow_01: solid_mat(&mut materials, [0.13, 0.07, 0.04]),
        bow_02: solid_mat(&mut materials, [0.26, 0.15, 0.08]),
        bow_03: solid_mat(&mut materials, [1.0, 1.0, 1.0]),
        barrel: solid_mat(&mut materials, [0.0, 0.93, 0.29]),
        box_green: solid_mat(&mut materials, [0.0, 0.93, 0.29]),
        binoculars: solid_mat(&mut materials, [1.0, 1.0, 1.0]),
        torch_01: solid_mat(&mut materials, [0.26, 0.15, 0.08]),
        torch_02: solid_mat(&mut materials, [0.86, 0.86, 0.86]),
        torch_03: solid_mat(&mut materials, [0.10, 0.08, 0.03]),
    };
    info!("[OVMAT] overlay material library ready (12 MIs)");
    commands.insert_resource(lib);
}

#[derive(Component)]
pub struct AvatarMaterialOverride(pub Vec<Handle<StandardMaterial>>);

#[derive(Component, Debug, Clone, Copy)]
pub struct AvatarMaterialApplied {
    pub slots: u32,
    pub body_slots: u32,
    pub main_slots: u32,
    pub source_slot_pattern: bool,
}

fn avatar_material_bridge(
    mut commands: Commands,
    avatars: Query<(
        Entity,
        &AvatarMaterialOverride,
        Option<&AvatarMaterialApplied>,
    )>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>, With<Mesh3d>>,
) {
    for (root, override_comp, applied) in &avatars {
        if applied.is_some() {
            continue;
        }
        let mut stack = vec![root];
        let mut mesh_entities = Vec::new();
        while let Some(e) = stack.pop() {
            if e != root && avatars.contains(e) {
                continue;
            }
            if let Ok(kids) = children.get(e) {
                for child in kids.iter().rev() {
                    stack.push(child);
                }
            }
            if mesh_materials.get(e).is_ok() {
                mesh_entities.push(e);
            }
        }
        if mesh_entities.is_empty() {
            continue;
        }
        let source_materials: Vec<_> = mesh_entities
            .iter()
            .filter_map(|entity| {
                mesh_materials
                    .get(*entity)
                    .ok()
                    .map(|material| material.0.id())
            })
            .collect();
        if source_materials.len() != mesh_entities.len() {
            continue;
        }
        let source_slot_pattern = source_materials.len() == override_comp.0.len()
            && source_materials.iter().enumerate().all(|(slot, source)| {
                source_materials[..slot]
                    .iter()
                    .enumerate()
                    .all(|(previous, other)| {
                        (*source == *other) == (override_comp.0[slot] == override_comp.0[previous])
                    })
            });
        let mut body_slots = 0u32;
        let mut main_slots = 0u32;
        for (slot, entity) in mesh_entities.iter().enumerate() {
            let material = override_comp
                .0
                .get(slot)
                .unwrap_or(override_comp.0.last().unwrap());
            commands
                .entity(*entity)
                .insert(MeshMaterial3d(material.clone()));
            if slot == AVATAR_MAIN_MATERIAL_SLOT {
                main_slots += 1;
            } else {
                body_slots += 1;
            }
        }
        if !source_slot_pattern {
            error!(
                "[OVMAT] avatar root={:?} source slot pattern mismatch slots={}",
                root,
                mesh_entities.len()
            );
        }
        commands.entity(root).insert(AvatarMaterialApplied {
            slots: mesh_entities.len() as u32,
            body_slots,
            main_slots,
            source_slot_pattern,
        });
        info!(
            "[OVMAT] avatar root={:?} replaced={} body={} main={} source_pattern={}",
            root,
            mesh_entities.len(),
            body_slots,
            main_slots,
            source_slot_pattern
        );
    }
}

#[derive(Component)]
pub struct OverlayPropMeshes(pub Vec<Handle<Mesh>>);

#[derive(Component)]
pub struct OverlayPropMaterials(pub Vec<Handle<StandardMaterial>>);

fn overlay_prop_material_bridge(
    mut commands: Commands,
    props: Query<(Entity, &OverlayPropMaterials, &Children), Added<OverlayPropMaterials>>,
    mesh_entities: Query<Entity, With<Mesh3d>>,
) {
    for (root, mats, children) in &props {
        let mut idx = 0usize;
        for child in children.iter() {
            if mesh_entities.contains(child) && idx < mats.0.len() {
                commands
                    .entity(child)
                    .insert(MeshMaterial3d(mats.0[idx].clone()));
                idx += 1;
            }
        }
        if idx > 0 {
            commands.entity(root).remove::<OverlayPropMaterials>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn nested_camera_materials_preserve_source_mesh_slot_ownership() {
        for camera_loads_first in [true, false] {
            let mut world = World::new();
            let mut materials = Assets::<StandardMaterial>::default();
            let source_body = materials.add(StandardMaterial::default());
            let source_main = materials.add(StandardMaterial::default());
            let target_body = materials.add(StandardMaterial::default());
            let target_main = materials.add(StandardMaterial::default());
            let mut slots = vec![target_body.clone(); 15];
            slots[AVATAR_MAIN_MATERIAL_SLOT] = target_main.clone();
            let body = world.spawn(AvatarMaterialOverride(slots)).id();
            let camera = world
                .spawn((
                    AvatarMaterialOverride(vec![target_main.clone()]),
                    ChildOf(body),
                ))
                .id();
            let mut body_meshes = Vec::new();
            let mut camera_mesh = None;
            for camera_ready in [camera_loads_first, !camera_loads_first] {
                if camera_ready {
                    camera_mesh = Some(
                        world
                            .spawn((
                                Mesh3d::default(),
                                MeshMaterial3d(source_main.clone()),
                                ChildOf(camera),
                            ))
                            .id(),
                    );
                } else {
                    for index in 0..15 {
                        let material = if index == AVATAR_MAIN_MATERIAL_SLOT {
                            source_main.clone()
                        } else {
                            source_body.clone()
                        };
                        body_meshes.push(
                            world
                                .spawn((Mesh3d::default(), MeshMaterial3d(material), ChildOf(body)))
                                .id(),
                        );
                    }
                }
                world.run_system_once(avatar_material_bridge).unwrap();
            }
            let body_applied = world.get::<AvatarMaterialApplied>(body).unwrap();
            assert_eq!(body_applied.slots, 15);
            assert!(body_applied.source_slot_pattern);
            assert_eq!((body_applied.body_slots, body_applied.main_slots), (14, 1));
            let camera_applied = world.get::<AvatarMaterialApplied>(camera).unwrap();
            assert_eq!(camera_applied.slots, 1);
            assert!(camera_applied.source_slot_pattern);
            for (index, mesh) in body_meshes.into_iter().enumerate() {
                let expected = if index == AVATAR_MAIN_MATERIAL_SLOT {
                    &target_main
                } else {
                    &target_body
                };
                assert_eq!(
                    &world
                        .get::<MeshMaterial3d<StandardMaterial>>(mesh)
                        .unwrap()
                        .0,
                    expected
                );
            }
            assert_eq!(
                world
                    .get::<MeshMaterial3d<StandardMaterial>>(camera_mesh.unwrap())
                    .unwrap()
                    .0,
                target_main
            );
        }
    }
}
