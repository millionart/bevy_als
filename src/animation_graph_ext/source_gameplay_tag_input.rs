use std::{cell::RefCell, collections::HashMap, sync::Arc};

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    id::BoneId,
    skeleton::Skeleton,
};
use bevy_gameplay_tag::GameplayTag;

use crate::extensions::{
    acl_animation::{AlsAclAnimation, AlsAclAnimations},
    als_curves::{AlsPoseCurves, ClipCurves, SourceCurveValues},
    source_pose::{SourceSkeletonLayout, SourceSkeletonLayouts},
    source_sequence_player::{AlsAnimationSharingSettings, SharedClipSamples},
};

#[derive(Clone, Default)]
struct SourceInputValues {
    tags: HashMap<String, GameplayTag>,
    slots: HashMap<String, crate::extensions::animation_context::AnimationContextState>,
    rig_query: Option<crate::control_rig_query::RigQueryContext>,
    rig_foot_locations_cm: Option<[Vec3; 2]>,
    foot_lock_distances_squared_cm: Option<[f64; 2]>,
    graphs: HashMap<String, LinkedGraphInput>,
}

#[derive(Clone, Debug)]
pub struct LinkedGraphInput {
    pub graph: Handle<AnimationGraph>,
    pub blend_in: f32,
    pub blend_out: f32,
}

#[derive(Component, Clone, Default)]
pub struct SourceAnimationInputs(Arc<SourceInputValues>, Option<ClipCurves>);

struct SourceEvaluation {
    entity: Entity,
    inputs: SourceAnimationInputs,
    skeleton_layouts: Option<Arc<SourceSkeletonLayouts>>,
    animation_lod: Option<crate::AlsAnimationLod>,
    clip_samples: Option<Arc<SharedClipSamples>>,
    acl_animations: Option<AlsAclAnimations>,
}

thread_local! {
    static EVALUATING: RefCell<Option<SourceEvaluation>> = const { RefCell::new(None) };
}

impl SourceAnimationInputs {
    pub(crate) fn pre_rig_foot_transition(
        &self,
        scale: f32,
        lock_amounts: [f32; 2],
    ) -> Option<Option<crate::als::transitions::TransitionSequence>> {
        use crate::als::{
            state::Stance,
            transitions::{TransitionConfig, select_dynamic_transition},
        };
        let distances = self.0.foot_lock_distances_squared_cm?;
        let config = TransitionConfig::default();
        Some(
            select_dynamic_transition(distances, lock_amounts, scale, Stance::Standing, &config)
                .map(|selected| selected.sequence),
        )
    }

    pub(crate) fn pre_rig_compatible(&self, other: &Self) -> bool {
        let a = &self.0;
        let b = &other.0;
        let same_curves = match (&self.1, &other.1) {
            (Some(a), Some(b)) => a.same_store(b),
            (None, None) => true,
            _ => false,
        };
        same_curves && a.slots.is_empty() && b.slots.is_empty()
            && a.tags == b.tags && a.graphs.len() == b.graphs.len()
            && a.graphs.iter().all(|(key, link)| {
                b.graphs.get(key).is_some_and(|other| {
                    link.graph == other.graph && link.blend_in.to_bits() == other.blend_in.to_bits()
                        && link.blend_out.to_bits() == other.blend_out.to_bits()
                })
            })
    }

    pub fn set_gameplay_tag_input(&mut self, key: impl Into<String>, tag: GameplayTag) {
        Arc::make_mut(&mut self.0).tags.insert(key.into(), tag);
    }

    pub fn gameplay_tag_input(&self, key: &str) -> Option<GameplayTag> {
        self.0.tags.get(key).cloned()
    }

    pub fn set_linked_graph_input(&mut self, key: &str, graph: Option<LinkedGraphInput>) {
        let values = Arc::make_mut(&mut self.0);
        if let Some(graph) = graph {
            values.graphs.insert(key.to_owned(), graph);
        } else {
            values.graphs.remove(key);
        }
    }

    pub(crate) fn linked_graph_ids(&self) -> impl Iterator<Item = AssetId<AnimationGraph>> + '_ {
        self.0.graphs.values().map(|link| link.graph.id())
    }

    pub fn linked_graph_input(&self, key: &str) -> Option<LinkedGraphInput> {
        self.0.graphs.get(key).cloned()
    }

    pub fn set_slot_input_context(
        &mut self,
        key: &str,
        context: Option<crate::extensions::animation_context::AnimationContextState>,
    ) {
        let values = Arc::make_mut(&mut self.0);
        if let Some(context) = context {
            values.slots.insert(key.to_owned(), context);
        } else {
            values.slots.remove(key);
        }
    }

    pub fn slot_input_contexts(
        &self,
    ) -> impl Iterator<Item = &crate::extensions::animation_context::AnimationContextState> {
        self.0.slots.values()
    }

    pub fn slot_input_context(
        &self,
        key: &str,
    ) -> Option<crate::extensions::animation_context::AnimationContextState> {
        self.0.slots.get(key).cloned()
    }

    pub fn set_rig_query_context(
        &mut self,
        context: Option<crate::control_rig_query::RigQueryContext>,
    ) {
        Arc::make_mut(&mut self.0).rig_query = context;
    }

    pub fn rig_query_context(&self) -> Option<crate::control_rig_query::RigQueryContext> {
        self.0.rig_query.clone()
    }

    pub(crate) fn set_rig_foot_locations_cm(&mut self, locations: Option<[Vec3; 2]>) {
        Arc::make_mut(&mut self.0).rig_foot_locations_cm = locations;
    }

    pub(crate) fn rig_foot_locations_cm(&self) -> Option<[Vec3; 2]> {
        self.0.rig_foot_locations_cm
    }

    pub(crate) fn set_foot_lock_distances_squared_cm(&mut self, distances: Option<[f64; 2]>) {
        Arc::make_mut(&mut self.0).foot_lock_distances_squared_cm = distances;
    }

    pub fn evaluate<T>(&self, entity: Entity, evaluate: impl FnOnce() -> T) -> T {
        self.evaluate_scoped(entity, None, None, None, None, evaluate)
    }

    pub(crate) fn evaluate_with_skeleton_layouts<T>(
        &self,
        entity: Entity,
        layouts: &Arc<SourceSkeletonLayouts>,
        lod: Option<&crate::AlsAnimationLod>,
        clip_samples: Option<&Arc<SharedClipSamples>>,
        acl_animations: Option<&AlsAclAnimations>,
        evaluate: impl FnOnce() -> T,
    ) -> T {
        self.evaluate_scoped(
            entity,
            Some(Arc::clone(layouts)),
            lod.cloned(),
            clip_samples.cloned(),
            acl_animations.cloned(),
            evaluate,
        )
    }

    fn evaluate_scoped<T>(
        &self,
        entity: Entity,
        skeleton_layouts: Option<Arc<SourceSkeletonLayouts>>,
        animation_lod: Option<crate::AlsAnimationLod>,
        clip_samples: Option<Arc<SharedClipSamples>>,
        acl_animations: Option<AlsAclAnimations>,
        evaluate: impl FnOnce() -> T,
    ) -> T {
        struct Restore(Option<SourceEvaluation>);
        impl Drop for Restore {
            fn drop(&mut self) {
                EVALUATING.with(|current| current.replace(self.0.take()));
            }
        }
        let _restore = Restore(EVALUATING.with(|current| {
            current.replace(Some(SourceEvaluation {
                entity,
                inputs: self.clone(),
                skeleton_layouts,
                animation_lod,
                clip_samples,
                acl_animations,
            }))
        }));
        evaluate()
    }
}

pub struct SourceAnimationInputsPlugin;

impl Plugin for SourceAnimationInputsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClipCurves>()
            .init_resource::<AlsAnimationSharingSettings>()
            .init_resource::<AlsAclAnimations>()
            .init_resource::<crate::AlsPreRigSharingSettings>()
            .register_required_components::<AnimationGraphPlayer, SourceAnimationInputs>()
            .add_observer(
                |event: On<Add<SourceAnimationInputs>>,
                 curves: Res<ClipCurves>,
                 mut inputs: Query<&mut SourceAnimationInputs>| {
                    if let Ok(mut inputs) = inputs.get_mut(event.entity) {
                        inputs.1 = Some(curves.clone());
                    }
                },
            )
            .add_observer(
                |event: On<Discard<AnimationGraphPlayer>>,
                 mut inputs: Query<&mut SourceAnimationInputs>| {
                    if let Ok(mut inputs) = inputs.get_mut(event.entity) {
                        inputs.0 = default();
                    }
                },
            );
    }
}

pub(crate) fn active_skeleton_layouts() -> Option<Arc<SourceSkeletonLayouts>> {
    EVALUATING.with_borrow(|active| active.as_ref()?.skeleton_layouts.clone())
}

fn current_inputs(entity: Entity) -> Option<SourceAnimationInputs> {
    EVALUATING.with_borrow(|current| {
        current
            .as_ref()
            .filter(|active| active.entity == entity)
            .map(|active| active.inputs.clone())
    })
}

pub(crate) fn with_skeleton_layout<T>(
    asset: AssetId<Skeleton>,
    inspect: impl FnOnce(&SourceSkeletonLayout) -> T,
) -> Option<T> {
    EVALUATING.with_borrow(|current| {
        let layout = current.as_ref()?.skeleton_layouts.as_ref()?.get(asset)?;
        Some(inspect(layout))
    })
}

pub(crate) fn active_omitted_bones(
    asset: AssetId<Skeleton>,
) -> Option<Arc<bevy::platform::collections::HashSet<BoneId>>> {
    EVALUATING.with_borrow(|current| {
        current.as_ref()?.animation_lod.as_ref()?.omitted_bone_plan(asset).cloned()
    })
}

pub(crate) fn active_clip_samples() -> Option<Arc<SharedClipSamples>> {
    EVALUATING.with_borrow(|current| current.as_ref()?.clip_samples.clone())
}

pub(crate) fn active_pre_rig_inputs(
    entity: Entity,
) -> Option<(Arc<SharedClipSamples>, SourceAnimationInputs)> {
    EVALUATING.with_borrow(|current| {
        let active = current.as_ref().filter(|active| active.entity == entity)?;
        let inputs = &active.inputs;
        if !inputs.0.slots.is_empty() || !inputs.0.graphs.values().all(|link| {
            link.graph.path().is_some_and(|path| {
                path.path().to_str().is_some_and(|name| {
                    name.starts_with("ALS/Character/AnimationInstances/")
                        && crate::als_asset_path(name) == *path
                })
            })
        }) {
            return None;
        }
        Some((active.clip_samples.clone()?, inputs.clone()))
    })
}

pub(crate) fn active_curve_filter() -> Option<Arc<crate::animation_lod::AlsAnimationCurveFilter>> {
    EVALUATING.with_borrow(|current| current.as_ref()?.animation_lod.as_ref()?.curve_filter.clone())
}

pub(crate) fn active_acl_animation(clip: &GraphClip) -> Option<Arc<AlsAclAnimation>> {
    EVALUATING.with_borrow(|current| current.as_ref()?.acl_animations.as_ref()?.get(clip))
}

pub(crate) fn active_acl_identity() -> usize {
    EVALUATING.with_borrow(|current| {
        current.as_ref().and_then(|active| active.acl_animations.as_ref())
            .map_or(0, AlsAclAnimations::cache_identity)
    })
}

pub(crate) fn active_lod_allows_control_rig(maximum_level: Option<usize>) -> bool {
    EVALUATING.with_borrow(|current| {
        current.as_ref().and_then(|active| active.animation_lod.as_ref())
            .is_none_or(|lod| lod.allows_control_rig(maximum_level))
    })
}

pub(crate) fn active_lod_allows_node(maximum_level: Option<usize>) -> bool {
    if maximum_level.is_none() {
        return true;
    }
    EVALUATING.with_borrow(|current| {
        current.as_ref().and_then(|active| active.animation_lod.as_ref())
            .is_none_or(|lod| lod.allows_node(maximum_level))
    })
}

pub(crate) fn sample_graph_clip_curves(
    entity: Entity,
    clip: Option<&GraphClip>,
    fallback: &AlsPoseCurves,
    time: f32,
) -> SourceCurveValues {
    clip.and_then(|clip| {
        active_acl_animation(clip)
            .and_then(|animation| animation.curve_values(time).ok().flatten())
            .or_else(|| current_inputs(entity)?.1?.sample(clip, time))
    })
        .unwrap_or_else(|| fallback.sample_all(time))
}

pub(crate) fn active_gameplay_tag_input(entity: Entity, key: &str) -> Option<GameplayTag> {
    current_inputs(entity)?.gameplay_tag_input(key)
}

pub(crate) fn active_linked_graph_input(entity: Entity, key: &str) -> Option<LinkedGraphInput> {
    current_inputs(entity)?.linked_graph_input(key)
}

pub(crate) fn active_slot_input_context(
    entity: Entity,
    key: &str,
) -> Option<crate::extensions::animation_context::AnimationContextState> {
    current_inputs(entity)?.slot_input_context(key)
}

pub(crate) fn active_rig_query_context(
    entity: Entity,
) -> Option<crate::control_rig_query::RigQueryContext> {
    current_inputs(entity)?.rig_query_context()
}

pub(crate) fn active_rig_foot_locations_cm(entity: Entity) -> Option<[Vec3; 2]> {
    current_inputs(entity)?.rig_foot_locations_cm()
}

pub(crate) fn active_foot_lock_distances_squared_cm(entity: Entity) -> Option<[f64; 2]> {
    current_inputs(entity)?.0.foot_lock_distances_squared_cm
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceGameplayTagInputNode {
    pub key: String,
    pub output: String,
    pub expected_tag: String,
    pub source_property_path: String,
    pub source_properties: Vec<String>,
}

impl SourceGameplayTagInputNode {
    fn evaluation_error(&self, ctx: &NodeContext, error: impl Into<String>) -> GraphError {
        GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id.clone(),
            error: error.into(),
        }
    }

    fn value(&self, entity: Entity) -> Result<bool, &'static str> {
        if self.key.is_empty() {
            return Err("gameplay tag key is empty");
        }
        let tag =
            active_gameplay_tag_input(entity, &self.key).ok_or("gameplay tag input is missing")?;
        if !self.expected_tag.is_empty() {
            return Ok(tag
                .as_str()
                .eq_ignore_ascii_case(GameplayTag::new(&self.expected_tag).as_str()));
        }
        Ok(tag.is_valid())
    }
}

impl NodeLike for SourceGameplayTagInputNode {
    fn display_name(&self) -> String {
        "ALS Source Gameplay Tag Input".to_owned()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.output.is_empty() {
            return Err(self.evaluation_error(&ctx, "gameplay tag output is empty"));
        }
        let value = self
            .value(ctx.graph_context.root_entity)
            .map_err(|error| self.evaluation_error(&ctx, error))?;
        ctx.set_data_fwd(self.output.clone(), value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        if self.output.is_empty() {
            return Err(GraphError::GlobalInputDataMissing(self.output.clone()));
        }
        ctx.add_output_data(self.output.clone(), DataSpec::Bool);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn browser_review_tag_input_compares_source_literal_ignoring_case() {
        let entity = Entity::PLACEHOLDER;
        let mut inputs = SourceAnimationInputs::default();
        inputs.set_gameplay_tag_input("browser_tag", GameplayTag::new("ALS.ROTATIONMODE.AIMING"));
        let node = SourceGameplayTagInputNode {
            key: "browser_tag".into(),
            output: "Value".into(),
            expected_tag: "Als.RotationMode.Aiming".into(),
            ..default()
        };
        inputs.evaluate(entity, || {
            assert_eq!(
                node.value(entity),
                Ok(true),
                "Source literal lost its case-insensitive identity"
            );
            assert_eq!(
                SourceGameplayTagInputNode {
                    expected_tag: "Als.RotationMode.VelocityDirection".into(),
                    ..node.clone()
                }
                .value(entity),
                Ok(false)
            );
        });
    }

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SourceAnimationInputsPlugin));
        app
    }

    #[test]
    fn input_namespaces_separate_worlds_with_identical_entity_ids() {
        let mut first = app();
        let mut second = app();
        let first_entity = first
            .world_mut()
            .spawn(AnimationGraphPlayer::default())
            .id();
        let second_entity = second
            .world_mut()
            .spawn(AnimationGraphPlayer::default())
            .id();
        assert_eq!(first_entity, second_entity);
        first
            .world_mut()
            .get_mut::<SourceAnimationInputs>(first_entity)
            .unwrap()
            .set_gameplay_tag_input("world_scope", GameplayTag::new("Fixture.First"));
        second
            .world_mut()
            .get_mut::<SourceAnimationInputs>(second_entity)
            .unwrap()
            .set_gameplay_tag_input("world_scope", GameplayTag::new("Fixture.Second"));
        let first = first
            .world()
            .get::<SourceAnimationInputs>(first_entity)
            .unwrap();
        let second = second
            .world()
            .get::<SourceAnimationInputs>(second_entity)
            .unwrap();
        let read = || {
            active_gameplay_tag_input(first_entity, "world_scope")
                .unwrap()
                .as_str()
                .to_owned()
        };
        first.evaluate(first_entity, || {
            assert_eq!(read(), "Fixture.First");
            assert_eq!(second.evaluate(second_entity, read), "Fixture.Second");
            assert_eq!(read(), "Fixture.First");
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                second.evaluate(second_entity, || panic!("Scope exit"))
            }));
            assert!(panic.is_err());
            assert_eq!(read(), "Fixture.First");
        });
        assert!(active_gameplay_tag_input(first_entity, "world_scope").is_none());
    }

    #[test]
    fn old_player_removal_cannot_clear_republished_inputs() {
        let mut app = app();
        let entity = app.world_mut().spawn(AnimationGraphPlayer::default()).id();
        for replace in [false, true] {
            let old = {
                let mut inputs = app
                    .world_mut()
                    .get_mut::<SourceAnimationInputs>(entity)
                    .unwrap();
                inputs.set_gameplay_tag_input("replacement", GameplayTag::new("Fixture.Old"));
                inputs.set_slot_input_context("slot", Some(default()));
                inputs.set_linked_graph_input(
                    "layer",
                    Some(LinkedGraphInput {
                        graph: default(),
                        blend_in: 0.2,
                        blend_out: 0.2,
                    }),
                );
                inputs.set_rig_query_context(Some(crate::control_rig_query::RigQueryContext {
                    world: Arc::new(crate::control_rig_query::RigQueryWorld::default()),
                    to_world: default(),
                    channel: "Visibility".into(),
                    filter: default(),
                }));
                Arc::downgrade(&inputs.0)
            };
            if !replace {
                app.world_mut()
                    .entity_mut(entity)
                    .remove::<AnimationGraphPlayer>();
            }
            app.world_mut()
                .entity_mut(entity)
                .insert(AnimationGraphPlayer::default());
            assert!(
                old.upgrade().is_none(),
                "Discard retained old instance inputs"
            );
            let inputs = app.world().get::<SourceAnimationInputs>(entity).unwrap();
            assert!(inputs.gameplay_tag_input("replacement").is_none());
            assert!(inputs.slot_input_context("slot").is_none());
            assert!(inputs.linked_graph_input("layer").is_none());
            assert!(inputs.rig_query_context().is_none());
            app.world_mut()
                .get_mut::<SourceAnimationInputs>(entity)
                .unwrap()
                .set_gameplay_tag_input("replacement", GameplayTag::new("Fixture.New"));
            app.update();
            assert_eq!(
                app.world()
                    .get::<SourceAnimationInputs>(entity)
                    .unwrap()
                    .gameplay_tag_input("replacement")
                    .unwrap()
                    .as_str(),
                "Fixture.New"
            );
        }
        let old = Arc::downgrade(&app.world().get::<SourceAnimationInputs>(entity).unwrap().0);
        app.world_mut().despawn(entity);
        assert!(old.upgrade().is_none());
    }

    #[test]
    fn source_gameplay_tag_input_uses_real_tag_identity() {
        let entity = Entity::PLACEHOLDER;
        let mut inputs = SourceAnimationInputs::default();
        inputs.set_gameplay_tag_input("get_parent.gait", GameplayTag::new("Als.Gait.Walking"));
        let node = SourceGameplayTagInputNode {
            key: "get_parent.gait".into(),
            output: "Value".into(),
            ..default()
        };
        inputs.evaluate(entity, || {
            assert_eq!(node.value(entity), Ok(true));
            assert_eq!(
                SourceGameplayTagInputNode {
                    expected_tag: "Als.Gait.Walking".into(),
                    ..node.clone()
                }
                .value(entity),
                Ok(true)
            );
            assert_eq!(
                SourceGameplayTagInputNode {
                    expected_tag: "Als.Gait.Sprinting".into(),
                    ..node.clone()
                }
                .value(entity),
                Ok(false)
            );
        });
        assert!(node.value(entity).is_err());
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<SourceGameplayTagInputNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SourceGameplayTagInputNode>())
                .unwrap()
                .data::<ReflectNodeLike>()
                .is_some()
        );
    }
}
