use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use bevy_gameplay_tag::GameplayTag;

#[derive(Default)]
struct SourceInputValues {
    tags: HashMap<String, GameplayTag>,
    slots: HashMap<String, crate::animation_context::AnimationContextState>,
    rig_query: Option<crate::control_rig_query::RigQueryContext>,
    graphs: HashMap<String, LinkedGraphInput>,
}

#[derive(Clone, Debug)]
pub struct LinkedGraphInput {
    pub graph: Handle<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    pub blend_in: f32,
    pub blend_out: f32,
}

type GameplayTagInputs = HashMap<Entity, SourceInputValues>;

static INPUTS: OnceLock<RwLock<GameplayTagInputs>> = OnceLock::new();

fn inputs() -> &'static RwLock<GameplayTagInputs> {
    INPUTS.get_or_init(Default::default)
}

pub fn set_linked_graph_input(entity: Entity, key: &str, graph: Option<LinkedGraphInput>) {
    let mut values = inputs().write().expect("source input registry is poisoned");
    if let Some(graph) = graph {
        values.entry(entity).or_default().graphs.insert(key.to_owned(), graph);
    } else if let Some(values) = values.get_mut(&entity) {
        values.graphs.remove(key);
    }
}

pub(crate) fn linked_graph_input(entity: Entity, key: &str) -> Option<LinkedGraphInput> {
    inputs().read().expect("source input registry is poisoned").get(&entity).and_then(|values| values.graphs.get(key)).cloned()
}

pub fn set_rig_query_context(entity: Entity, context: Option<crate::control_rig_query::RigQueryContext>) {
    let mut inputs = inputs().write().expect("source input registry is poisoned");
    if context.is_some() {
        inputs.entry(entity).or_default().rig_query = context;
    } else if let Some(values) = inputs.get_mut(&entity) {
        values.rig_query = None;
    }
}

pub fn rig_query_context(entity: Entity) -> Option<crate::control_rig_query::RigQueryContext> {
    inputs().read().expect("source input registry is poisoned").get(&entity)
        .and_then(|values| values.rig_query.clone())
}

pub fn set_gameplay_tag_input(entity: Entity, key: impl Into<String>, tag: GameplayTag) {
    inputs()
        .write()
        .expect("gameplay tag input registry is poisoned")
        .entry(entity)
        .or_default()
        .tags
        .insert(key.into(), tag);
}

pub fn set_slot_input_context(
    entity: Entity,
    key: &str,
    context: Option<crate::animation_context::AnimationContextState>,
) {
    let mut values = inputs().write().expect("source input registry is poisoned");
    let values = values.entry(entity).or_default();
    if let Some(context) = context {
        values.slots.insert(key.to_owned(), context);
    } else {
        values.slots.remove(key);
    }
}

pub fn slot_input_context(
    entity: Entity,
    key: &str,
) -> Option<crate::animation_context::AnimationContextState> {
    inputs()
        .read()
        .expect("source input registry is poisoned")
        .get(&entity)
        .and_then(|values| values.slots.get(key))
        .cloned()
}

pub(crate) fn clear_gameplay_tag_inputs(entity: Entity) {
    inputs()
        .write()
        .expect("gameplay tag input registry is poisoned")
        .remove(&entity);
}

pub fn gameplay_tag_input(entity: Entity, key: &str) -> Option<GameplayTag> {
    inputs()
        .read()
        .expect("gameplay tag input registry is poisoned")
        .get(&entity)
        .and_then(|values| values.tags.get(key))
        .cloned()
}

pub(crate) fn clear_removed_gameplay_tag_inputs(
    mut removed: RemovedComponents<
        bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    >,
) {
    for entity in removed.read() {
        clear_gameplay_tag_inputs(entity);
    }
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
        let tag = gameplay_tag_input(entity, &self.key).ok_or("gameplay tag input is missing")?;
        if !self.expected_tag.is_empty() {
            return Ok(tag.matches_tag_exact(&GameplayTag::new(&self.expected_tag)));
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
    use super::*;

    #[test]
    fn source_gameplay_tag_input_uses_real_tag_identity() {
        use bevy::reflect::TypeRegistry;

        let entity = Entity::from_raw_u32(991).unwrap();
        set_gameplay_tag_input(
            entity,
            "get_parent.gait",
            GameplayTag::new("Als.Gait.Walking"),
        );
        let node = SourceGameplayTagInputNode {
            key: "get_parent.gait".to_owned(),
            output: "Value".to_owned(),
            ..default()
        };
        assert_eq!(node.value(entity), Ok(true));
        let expected = SourceGameplayTagInputNode {
            expected_tag: "Als.Gait.Walking".to_owned(),
            ..node.clone()
        };
        assert_eq!(expected.value(entity), Ok(true));
        let mismatch = SourceGameplayTagInputNode {
            expected_tag: "Als.Gait.Sprinting".to_owned(),
            ..node
        };
        assert_eq!(mismatch.value(entity), Ok(false));
        set_slot_input_context(entity, "slot.pose", Some(Default::default()));
        assert!(slot_input_context(entity, "slot.pose").is_some());
        let query = crate::control_rig_query::RigQueryContext {
            world: std::sync::Arc::new(crate::control_rig_query::RigQueryWorld {
                trees: default(), colliders: default(),
            }), to_world: default(), channel: "Visibility".into(), filter: default(),
        };
        set_rig_query_context(entity, Some(query.clone()));
        assert!(rig_query_context(entity).is_some());
        set_rig_query_context(entity, None);
        assert!(rig_query_context(entity).is_none());
        assert!(slot_input_context(entity, "slot.pose").is_some());
        set_rig_query_context(entity, Some(query));
        clear_gameplay_tag_inputs(entity);
        assert!(rig_query_context(entity).is_none());
        assert!(gameplay_tag_input(entity, "get_parent.gait").is_none());
        assert!(slot_input_context(entity, "slot.pose").is_none());
        let mut registry = TypeRegistry::default();
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
