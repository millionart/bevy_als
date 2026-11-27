use std::sync::Arc;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_animation_graph::core::{
    animated_scene::{AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance},
    animation_graph_player::AnimationGraphPlayer,
    id::BoneId,
    plugin::AnimationGraphSet,
    pose::Pose,
    ragdoll::bone_mapping::RagdollBoneMap,
    skeleton::Skeleton,
};

use crate::extensions::SourceCurveName;
use crate::{
    als::roll::AlsRollState, character_assembly::AlsCharacterAvatar,
    extensions::source_sequence_player::resolve_virtual_bones, get_up::AlsGetUpState,
    mantle_drive::AlsMantleMontages, ragdoll_link::AlsRagdollLink,
};

/// A lower-detail animation tier selected by its minimum camera distance, in meters.
#[derive(Clone, Debug)]
pub struct AlsAnimationLodTier {
    pub minimum_distance: f32,
    pub remove_branches: Vec<String>,
    pub evaluate_control_rig: bool,
}

/// With no active 3D camera or no eligible tier, the full skeleton is evaluated.
#[derive(Resource, Clone, Debug)]
pub struct AlsAnimationLodSettings {
    pub enabled: bool,
    pub hysteresis_meters: f32,
    /// Additional sockets or animation consumers that need their bones at every tier.
    pub required_bones: Vec<String>,
    /// Optional component allow-list for scalar pose curves.
    pub allow_curves: Option<Vec<SourceCurveName>>,
    pub disabled_curves: Vec<SourceCurveName>,
    pub curve_lod: Vec<AlsAnimationCurveLod>,
    /// Tiers in order of decreasing detail. Full detail is implicit at index zero.
    pub tiers: Vec<AlsAnimationLodTier>,
}

impl Default for AlsAnimationLodSettings {
    fn default() -> Self {
        let fingers = ["thumb", "index", "middle", "ring", "pinky"]
            .into_iter()
            .flat_map(|finger| [format!("{finger}_01_l"), format!("{finger}_01_r")])
            .collect::<Vec<_>>();
        Self {
            enabled: true,
            hysteresis_meters: 2.0,
            required_bones: Vec::new(),
            allow_curves: None,
            disabled_curves: Vec::new(),
            curve_lod: Vec::new(),
            tiers: vec![
                AlsAnimationLodTier {
                    minimum_distance: 32.0,
                    remove_branches: fingers.clone(),
                    evaluate_control_rig: true,
                },
                AlsAnimationLodTier {
                    minimum_distance: 72.0,
                    remove_branches: fingers,
                    evaluate_control_rig: false,
                },
            ],
        }
    }
}

/// Allows a named curve through its maximum LOD and while any linked bone remains required.
#[derive(Clone, Debug)]
pub struct AlsAnimationCurveLod {
    pub curve: SourceCurveName,
    pub maximum_level: Option<usize>,
    pub linked_bones: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct AlsAnimationCurveFilter {
    allowed: Option<HashSet<SourceCurveName>>,
    disabled: HashSet<SourceCurveName>,
}

impl AlsAnimationCurveFilter {
    pub(crate) fn allows(&self, curve: &SourceCurveName) -> bool {
        self.allowed
            .as_ref()
            .is_none_or(|allowed| allowed.contains(curve))
            && !self.disabled.contains(curve)
    }
}

#[derive(Component, Clone, Debug)]
pub struct AlsAnimationLod {
    level: usize,
    evaluate_control_rig: bool,
    pub(crate) budget_reduced_rig: bool,
    omitted: Option<(AssetId<Skeleton>, Arc<HashSet<BoneId>>)>,
    pub(crate) curve_filter: Option<Arc<AlsAnimationCurveFilter>>,
    camera_distance: Option<f32>,
    requires_continuous_pose: bool,
}

impl Default for AlsAnimationLod {
    fn default() -> Self {
        Self {
            level: 0,
            evaluate_control_rig: true,
            budget_reduced_rig: false,
            omitted: None,
            curve_filter: None,
            camera_distance: None,
            requires_continuous_pose: false,
        }
    }
}

impl AlsAnimationLod {
    pub fn level(&self) -> usize {
        self.level
    }

    pub(crate) fn camera_distance(&self) -> Option<f32> {
        self.camera_distance
    }

    pub(crate) fn requires_continuous_pose(&self) -> bool {
        self.requires_continuous_pose
    }

    pub(crate) fn allows_control_rig(&self, maximum_level: Option<usize>) -> bool {
        self.evaluate_control_rig && !self.budget_reduced_rig && self.allows_node(maximum_level)
    }

    pub(crate) fn allows_node(&self, maximum_level: Option<usize>) -> bool {
        maximum_level.is_none_or(|maximum| self.level <= maximum)
    }

    pub(crate) fn omitted_bones(&self, skeleton: AssetId<Skeleton>) -> Option<&HashSet<BoneId>> {
        self.omitted_bone_plan(skeleton).map(Arc::as_ref)
    }

    pub(crate) fn omitted_bone_plan(
        &self,
        skeleton: AssetId<Skeleton>,
    ) -> Option<&Arc<HashSet<BoneId>>> {
        self.omitted
            .as_ref()
            .and_then(|(asset, required)| (*asset == skeleton).then_some(required))
    }

    /// Applies the current bone selection, returning a compact pose when channels are omitted.
    pub fn compact_pose(&self, pose: &Pose) -> Option<Pose> {
        let omitted = self.omitted_bones(pose.skeleton.id())?;
        if pose.paths.keys().all(|bone| !omitted.contains(bone)) {
            return None;
        }
        let mut compact = Pose {
            timestamp: pose.timestamp,
            skeleton: pose.skeleton.clone(),
            root_motion: pose.root_motion.clone(),
            ..default()
        };
        compact
            .bones
            .reserve(pose.bones.len().saturating_sub(omitted.len()));
        compact
            .paths
            .reserve(pose.paths.len().saturating_sub(omitted.len()));
        for (&bone, &index) in &pose.paths {
            if !omitted.contains(&bone) {
                compact.add_bone(pose.bones[index].clone(), bone);
            }
        }
        Some(compact)
    }
}

pub(crate) struct AlsAnimationLodPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AlsAnimationLodSet;

impl Plugin for AlsAnimationLodPlugin {
    fn build(&self, app: &mut App) {
        crate::extensions::post_physics_animation::require_animation_player_component::<AlsAnimationLod>(app);
        app.init_resource::<AlsAnimationLodSettings>()
            .add_observer(
                |event: On<Discard<AnimationGraphPlayer>>, mut commands: Commands| {
                    commands
                        .entity(event.entity)
                        .try_remove::<AlsAnimationLod>();
                },
            )
            .add_systems(
                PostUpdate,
                select_animation_lod
                    .in_set(AlsAnimationLodSet)
                    .before(AnimationGraphSet::PrePhysics),
            );
    }
}

struct LodPlan {
    omitted: Option<Arc<HashSet<BoneId>>>,
    curve_filter: Option<Arc<AlsAnimationCurveFilter>>,
}

type LodBonePlans = HashMap<(AssetId<Skeleton>, Option<AssetId<RagdollBoneMap>>, usize), LodPlan>;

fn select_animation_lod(
    settings: Res<AlsAnimationLodSettings>,
    skeletons: Res<Assets<Skeleton>>,
    scenes: Option<Res<Assets<AnimatedScene>>>,
    ragdoll_maps: Option<Res<Assets<RagdollBoneMap>>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    avatars: Query<(
        &AlsCharacterAvatar,
        &AnimatedSceneInstance,
        Option<&AnimatedSceneHandle>,
    )>,
    owners: Query<(
        &GlobalTransform,
        Option<&AlsRollState>,
        Has<AlsGetUpState>,
        Option<&AlsMantleMontages>,
        Option<&AlsRagdollLink>,
    )>,
    mut players: Query<(&AnimationGraphPlayer, &mut AlsAnimationLod)>,
    mut plans: Local<LodBonePlans>,
) {
    if settings.is_changed()
        || skeletons.is_changed()
        || ragdoll_maps.as_ref().is_some_and(|maps| maps.is_changed())
    {
        plans.clear();
    }
    for (avatar, scene, scene_handle) in &avatars {
        let Ok((player, mut lod)) = players.get_mut(scene.player_entity()) else {
            continue;
        };
        let Ok((transform, roll, getting_up, mantle, ragdoll)) = owners.get(avatar.character)
        else {
            continue;
        };
        let needs_full_pose = getting_up
            || roll.is_some_and(|roll| roll.active)
            || mantle.is_some_and(|mantle| !mantle.entries.is_empty())
            || ragdoll.is_some_and(|ragdoll| ragdoll.prev_ragdoll);
        let distance = cameras
            .iter()
            .filter(|(camera, _)| camera.is_active)
            .map(|(_, view)| view.translation().distance(transform.translation()))
            .filter(|distance| distance.is_finite())
            .reduce(f32::min);
        if lod.camera_distance != distance || lod.requires_continuous_pose != needs_full_pose {
            lod.camera_distance = distance;
            lod.requires_continuous_pose = needs_full_pose;
        }
        let level = if settings.enabled && !needs_full_pose {
            distance.map_or(0, |distance| choose_lod(&settings, lod.level, distance))
        } else {
            0
        };
        let tier = level
            .checked_sub(1)
            .and_then(|index| settings.tiers.get(index));
        let filter_curves = settings.allow_curves.is_some()
            || !settings.disabled_curves.is_empty()
            || !settings.curve_lod.is_empty();
        if tier.is_none() && !filter_curves {
            if lod.level != 0 || lod.curve_filter.is_some() {
                *lod = AlsAnimationLod {
                    camera_distance: distance,
                    requires_continuous_pose: needs_full_pose,
                    ..default()
                };
            }
            continue;
        }
        let asset = player.skeleton().id();
        let Some(skeleton) = skeletons.get(asset) else {
            *lod = AlsAnimationLod {
                camera_distance: distance,
                requires_continuous_pose: needs_full_pose,
                ..default()
            };
            continue;
        };
        let ragdoll_map_handle = scene_handle
            .and_then(|source| scenes.as_ref()?.get(&source.handle))
            .and_then(|scene| scene.ragdoll_bone_map.as_ref());
        let body_map = ragdoll_map_handle
            .and_then(|handle| ragdoll_maps.as_ref()?.get(handle))
            .filter(|body_map| body_map.skeleton.id() == asset);
        let plan = plans
            .entry((asset, ragdoll_map_handle.map(Handle::id), level))
            .or_insert_with(|| {
                let omitted = tier.map(|tier| {
                    let body_bones = body_map.into_iter().flat_map(|map| {
                        map.bodies_from_bones
                            .values()
                            .map(|body| body.bone.bone.id())
                    });
                    let required = required_bones(
                        skeleton,
                        tier,
                        body_bones.chain(bones_named(skeleton, &settings.required_bones)),
                    );
                    Arc::new(
                        skeleton
                            .iter_bones()
                            .filter(|bone| {
                                skeleton.default_transforms(*bone).is_some()
                                    && !required.contains(bone)
                            })
                            .collect::<HashSet<_>>(),
                    )
                });
                let curve_filter = filter_curves.then(|| {
                    Arc::new(compile_curve_filter(
                        &settings,
                        skeleton,
                        level,
                        omitted.as_deref(),
                    ))
                });
                LodPlan {
                    omitted,
                    curve_filter,
                }
            });
        let evaluate_control_rig = tier.is_none_or(|tier| tier.evaluate_control_rig);
        if lod.level != level
            || lod.evaluate_control_rig != evaluate_control_rig
            || lod
                .omitted
                .as_ref()
                .map(|(asset, omitted)| (*asset, Arc::as_ptr(omitted)))
                != plan
                    .omitted
                    .as_ref()
                    .map(|omitted| (asset, Arc::as_ptr(omitted)))
            || lod.curve_filter.as_ref().map(Arc::as_ptr)
                != plan.curve_filter.as_ref().map(Arc::as_ptr)
        {
            *lod = AlsAnimationLod {
                level,
                evaluate_control_rig,
                budget_reduced_rig: lod.budget_reduced_rig,
                omitted: plan
                    .omitted
                    .as_ref()
                    .map(|omitted| (asset, Arc::clone(omitted))),
                curve_filter: plan.curve_filter.clone(),
                camera_distance: distance,
                requires_continuous_pose: needs_full_pose,
            };
        }
    }
}

fn bones_named<'a>(
    skeleton: &'a Skeleton,
    names: &'a [String],
) -> impl Iterator<Item = BoneId> + 'a {
    skeleton.iter_paths().filter_map(|path| {
        let name = path.parts.last()?;
        names
            .iter()
            .any(|bone| name.as_str().eq_ignore_ascii_case(bone))
            .then(|| path.id())
    })
}

fn compile_curve_filter(
    settings: &AlsAnimationLodSettings,
    skeleton: &Skeleton,
    level: usize,
    omitted: Option<&HashSet<BoneId>>,
) -> AlsAnimationCurveFilter {
    let lod_disabled = settings.curve_lod.iter().filter(|rule| {
        rule.maximum_level.is_some_and(|maximum| level > maximum)
            || !rule.linked_bones.is_empty()
                && !bones_named(skeleton, &rule.linked_bones)
                    .any(|bone| omitted.is_none_or(|omitted| !omitted.contains(&bone)))
    });
    AlsAnimationCurveFilter {
        allowed: settings
            .allow_curves
            .as_ref()
            .map(|names| names.iter().cloned().collect()),
        disabled: settings
            .disabled_curves
            .iter()
            .cloned()
            .chain(lod_disabled.map(|rule| rule.curve.clone()))
            .collect(),
    }
}

fn choose_lod(settings: &AlsAnimationLodSettings, current: usize, distance: f32) -> usize {
    settings
        .tiers
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, tier)| {
            let level = index + 1;
            let hysteresis = if level <= current {
                settings.hysteresis_meters.max(0.0)
            } else {
                0.0
            };
            let threshold = (tier.minimum_distance - hysteresis).max(0.0);
            (tier.minimum_distance.is_finite() && distance >= threshold).then_some(level)
        })
        .unwrap_or(0)
}

fn required_bones(
    skeleton: &Skeleton,
    tier: &AlsAnimationLodTier,
    protected: impl IntoIterator<Item = BoneId>,
) -> HashSet<BoneId> {
    let mut required: HashSet<_> = skeleton
        .iter_paths()
        .filter(|path| {
            !path.parts.is_empty()
                && !path.parts.iter().any(|part| {
                    tier.remove_branches
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(part.as_str()))
                })
        })
        .map(|path| path.id())
        .collect();
    let require = |required: &mut HashSet<BoneId>, bone: BoneId| {
        let mut pending = Some(bone);
        while let Some(id) = pending {
            if !skeleton.has_id(&id) || !required.insert(id) {
                break;
            }
            pending = skeleton.parent(&id);
        }
    };
    require(&mut required, skeleton.root());
    for bone in protected {
        require(&mut required, bone);
    }
    let virtual_bones = resolve_virtual_bones(skeleton);
    loop {
        let previous = required.len();
        for binding in &virtual_bones {
            if required.contains(&binding.bone) {
                require(&mut required, binding.source);
                require(&mut required, binding.target);
            }
        }
        if required.len() == previous {
            return required;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_animation_graph::core::animation_clip::EntityPath;

    #[test]
    fn authored_node_limits_forward_the_base_without_querying_overlays() {
        use crate::extensions::{
            SourceAnimationInputs, SourceApplyAdditiveNode, SourceLayeredBoneBlendNode,
            SourceMeshSpaceAdditiveNode, source_pose::SourceSkeletonLayouts,
        };
        use bevy_animation_graph::core::{
            animation_graph::{GraphInputPin, TimeUpdate},
            animation_node::NodeLike,
            utils::testing::GraphTestHarness,
        };

        fn check(node: impl NodeLike, base: &str, overlay: &str) {
            let pose = Pose {
                timestamp: 0.25,
                ..default()
            };
            GraphTestHarness::node(node)
                .with_time_fwd(Some(TimeUpdate::Absolute(0.25)))
                .with_const_input_data(GraphInputPin::Passthrough(base.to_owned()), pose.clone())
                .with_input_data(GraphInputPin::Passthrough(overlay.to_owned()), |_| {
                    panic!("culled overlay evaluated")
                })
                .when_queried()
                .then_output_is("Pose", pose);
        }

        let lod = AlsAnimationLod {
            level: 1,
            ..default()
        };
        let layouts = Arc::new(SourceSkeletonLayouts::default());
        SourceAnimationInputs::default().evaluate_with_skeleton_layouts(
            Entity::PLACEHOLDER,
            &layouts,
            Some(&lod),
            None,
            None,
            || {
                check(
                    SourceApplyAdditiveNode {
                        use_constant_alpha: true,
                        constant_alpha: 1.0,
                        lod_threshold: Some(0),
                        ..default()
                    },
                    "Base",
                    "Additive",
                );
                check(
                    SourceMeshSpaceAdditiveNode {
                        use_constant_alpha: true,
                        constant_alpha: 1.0,
                        lod_threshold: Some(0),
                        ..default()
                    },
                    "Base",
                    "Additive",
                );
                check(
                    SourceLayeredBoneBlendNode {
                        lod_threshold: Some(0),
                        ..default()
                    },
                    "BasePose",
                    "BlendPoses_0",
                );
            },
        );
    }

    #[test]
    fn curve_lod_filters_both_runtime_codecs_and_reuses_required_bone_anchors() {
        use crate::extensions::{
            AlsPoseCurves, SourceAnimationInputs,
            source_gameplay_tag_input::sample_graph_clip_curves,
            source_pose::SourceSkeletonLayouts,
        };
        use bevy_animation_graph::core::animation_clip::{GraphClip, loader::GraphClipSource};

        let (app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let world = app.world();
        let player = world
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let clips = world.resource::<Assets<GraphClip>>();
        let clip = clips
            .iter()
            .find_map(|(_, clip)| match clip.source.as_ref() {
                Some(GraphClipSource::GltfNamed { animation_name, .. })
                    if animation_name == "A_Als_Walk_Forward_0" =>
                {
                    Some(clip)
                }
                _ => None,
            })
            .unwrap();
        let skeletons = world.resource::<Assets<Skeleton>>();
        let layouts = Arc::new(SourceSkeletonLayouts::new(skeletons));
        let skeleton = skeletons.get(&clip.skeleton).unwrap();
        let foot_planted = SourceCurveName::from("footplanted");
        let pose_gait = SourceCurveName::from("PoseGait");
        let linked = vec!["thumb_03_l".to_owned()];
        let thumb = bones_named(skeleton, &linked).next().unwrap();
        let settings = AlsAnimationLodSettings {
            curve_lod: vec![AlsAnimationCurveLod {
                curve: foot_planted.clone(),
                maximum_level: Some(1),
                linked_bones: linked,
            }],
            ..default()
        };
        let omitted = HashSet::from([thumb]);
        let filter = |level, omitted| compile_curve_filter(&settings, skeleton, level, omitted);
        assert!(filter(0, None).allows(&foot_planted));
        assert!(!filter(1, Some(&omitted)).allows(&foot_planted));
        assert!(filter(1, None).allows(&foot_planted));
        assert!(!filter(2, None).allows(&foot_planted));
        let lod = AlsAnimationLod {
            level: 1,
            curve_filter: Some(Arc::new(filter(1, Some(&omitted)))),
            ..default()
        };
        let inputs = world.get::<SourceAnimationInputs>(player).unwrap();
        for codec in [None, Some(world.resource::<crate::AlsAclAnimations>())] {
            for lod in [None, Some(&lod)] {
                let curves = inputs.evaluate_with_skeleton_layouts(
                    player,
                    &layouts,
                    lod,
                    None,
                    codec,
                    || sample_graph_clip_curves(player, Some(clip), &AlsPoseCurves::default(), 0.0),
                );
                assert_eq!(curves.contains_key(&foot_planted), lod.is_none());
                assert_eq!(curves[&pose_gait], 1.0);
            }
        }
    }

    #[test]
    fn tiers_are_stable_near_the_camera_boundary() {
        let settings = AlsAnimationLodSettings::default();
        assert_eq!(choose_lod(&settings, 0, 31.0), 0);
        assert_eq!(choose_lod(&settings, 0, 32.1), 1);
        assert_eq!(choose_lod(&settings, 1, 31.0), 1);
        assert_eq!(choose_lod(&settings, 1, 29.9), 0);
        assert_eq!(choose_lod(&settings, 1, 80.0), 2);
    }

    #[test]
    fn excludes_descendants_and_restores_required_physics_ancestors() {
        let root = EntityPath::default().child("root");
        let finger = root.child("index_01_l");
        let physics = finger.child("physics_joint");
        let tip = finger.child("index_03_l");
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root.id());
        for bone in [&root, &finger, &physics, &tip] {
            skeleton.add_bone(bone.clone(), Transform::IDENTITY, Transform::IDENTITY);
        }
        let settings = AlsAnimationLodSettings::default();
        let tier = &settings.tiers[0];
        let reduced = required_bones(&skeleton, tier, []);
        assert_eq!(reduced.len(), 1);
        let protected = required_bones(&skeleton, tier, [physics.id()]);
        assert!(protected.contains(&root.id()));
        assert!(protected.contains(&finger.id()));
        assert!(protected.contains(&physics.id()));
        assert!(!protected.contains(&tip.id()));
    }

    #[test]
    fn compaction_preserves_pose_channels_outside_the_skeleton() {
        let omitted_bone = EntityPath::default().child("Finger").id();
        let control = EntityPath::default().child("Control").id();
        let mut pose = Pose::default();
        pose.add_bone(default(), omitted_bone);
        pose.add_bone(default(), control);
        let lod = AlsAnimationLod {
            level: 1,
            evaluate_control_rig: true,
            omitted: Some((pose.skeleton.id(), Arc::new(HashSet::from([omitted_bone])))),
            ..default()
        };
        let compact = lod.compact_pose(&pose).unwrap();
        assert!(compact.get_bone(control).is_some());
        assert!(compact.get_bone(omitted_bone).is_none());
    }

    #[test]
    fn a_live_camera_changes_required_bones_and_suspends_the_rig() {
        use crate::extensions::{
            SourceAnimationInputs, root_graph_boundaries::current_control_rig,
            source_pose::SourceSkeletonLayouts, source_sequence_player::sample_pose,
        };
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::AnimationGraph,
        };

        let (mut app, owner, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let world = app.world_mut();
        world.entity_mut(avatar).insert(
            crate::animation_visibility::AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones,
        );
        let player = world
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let mut existing = world.query::<&mut Camera>();
        for mut camera in existing.iter_mut(world) {
            camera.is_active = false;
        }
        let view = world.spawn(Camera3d::default()).id();
        let update_view = |app: &mut App, distance| {
            let position = app
                .world()
                .get::<GlobalTransform>(owner)
                .unwrap()
                .translation();
            let transform = Transform::from_translation(position + Vec3::Z * distance);
            app.world_mut()
                .entity_mut(view)
                .insert((transform, GlobalTransform::from(transform)));
            app.update();
            app.update();
            let world = app.world();
            let player_state = world.get::<AnimationGraphPlayer>(player).unwrap();
            assert!(
                player_state.get_error().is_none(),
                "{:?}",
                player_state.get_error()
            );
            (
                world.get::<AlsAnimationLod>(player).unwrap().level(),
                player_state
                    .get_default_output_pose()
                    .unwrap()
                    .paths
                    .keys()
                    .copied()
                    .collect::<HashSet<_>>(),
                current_control_rig(player_state, world.resource::<Assets<AnimationGraph>>())
                    .is_some(),
                player_state.get_context_arena().unwrap().frame_index(),
            )
        };
        let near = update_view(&mut app, 3.0);
        assert_eq!((near.0, near.2), (0, true));
        let far = update_view(&mut app, 40.0);
        assert_eq!((far.0, far.2), (1, true));
        assert!(far.1.is_subset(&near.1));

        let world = app.world();
        let lod = world.get::<AlsAnimationLod>(player).unwrap();
        let inputs = world.get::<SourceAnimationInputs>(player).unwrap();
        let skeletons = world.resource::<Assets<Skeleton>>();
        let layouts = Arc::new(SourceSkeletonLayouts::new(skeletons));
        let clips = world.resource::<Assets<GraphClip>>();
        let clip = clips
            .iter()
            .find_map(|(_, clip)| {
                (clip.curves().len() == 79 && lod.omitted_bones(clip.skeleton.id()).is_some())
                    .then_some(clip)
            })
            .expect("the installed mannequin has a full clip for the active skeleton");
        let omitted = lod.omitted_bones(clip.skeleton.id()).unwrap();
        assert!(far.1.is_disjoint(omitted));
        let near_animated_bones = near
            .1
            .iter()
            .filter(|id| clip.curves().contains_key(&id.animation_target_id()))
            .count();
        let far_animated_bones = far
            .1
            .iter()
            .filter(|id| clip.curves().contains_key(&id.animation_target_id()))
            .count();
        assert_eq!((near_animated_bones, far_animated_bones), (79, 49));
        let clip_bones: HashSet<_> = clip.curves().keys().copied().map(BoneId::from).collect();
        let sampled =
            inputs.evaluate_with_skeleton_layouts(player, &layouts, Some(lod), None, None, || {
                sample_pose(clip, 0.2)
            });
        assert_eq!(sampled.bones.len(), 49);

        let distant = update_view(&mut app, 80.0);
        assert_eq!((distant.0, distant.2), (2, false));
        let returned = update_view(&mut app, 3.0);
        assert_eq!((returned.0, returned.2), (0, true));
        assert_eq!(
            returned
                .1
                .iter()
                .filter(|id| clip_bones.contains(*id))
                .count(),
            near_animated_bones,
        );
        assert!(near.3 < far.3 && far.3 < distant.3 && distant.3 < returned.3);
    }
}
