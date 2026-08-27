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
            && source_materials.len() > AVATAR_MAIN_MATERIAL_SLOT
            && source_materials[0] != source_materials[AVATAR_MAIN_MATERIAL_SLOT]
            && source_materials.iter().enumerate().all(|(slot, source)| {
                *source
                    == if slot == AVATAR_MAIN_MATERIAL_SLOT {
                        source_materials[AVATAR_MAIN_MATERIAL_SLOT]
                    } else {
                        source_materials[0]
                    }
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
