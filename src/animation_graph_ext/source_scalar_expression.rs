use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::extensions::animation_context::optional_data_back;

#[derive(Reflect, Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceScalarDataType {
    #[default]
    F32,
    Bool,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceScalarPin {
    pub name: String,
    pub data_type: SourceScalarDataType,
    pub f32_default: f32,
    pub bool_default: bool,
    pub source_default: String,
    pub global_key: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceScalarExpressionNode {
    pub input_pins: Vec<SourceScalarPin>,
    pub output_pins: Vec<SourceScalarPin>,
    pub operation: String,
    pub source_class: String,
    pub source_properties: Vec<String>,
}

fn bool_to_f32(value: bool) -> f32 {
    value as u8 as f32
}

impl SourceScalarExpressionNode {
    fn first_bool_input_name(&self) -> Option<&str> {
        self.input_pins
            .iter()
            .find(|pin| pin.data_type == SourceScalarDataType::Bool)
            .map(|pin| pin.name.as_str())
    }

    fn bool_input_pair(&self, ctx: &NodeContext) -> Result<Option<(bool, bool)>, GraphError> {
        let mut pins = self
            .input_pins
            .iter()
            .filter(|pin| pin.data_type == SourceScalarDataType::Bool);
        let Some((first, second)) = pins.next().zip(pins.next()) else {
            return Ok(None);
        };
        Ok(Some((
            self.input_bool(ctx, &first.name)?,
            self.input_bool(ctx, &second.name)?,
        )))
    }

    fn input_f32(&self, ctx: &NodeContext, name: &str) -> Result<f32, GraphError> {
        let fallback = self
            .input_pins
            .iter()
            .find(|pin| pin.name == name)
            .map(|pin| pin.f32_default)
            .unwrap_or_default();
        optional_data_back(ctx, name)?.map_or(Ok(fallback), |value| value.as_f32())
    }

    fn input_bool(&self, ctx: &NodeContext, name: &str) -> Result<bool, GraphError> {
        let fallback = self
            .input_pins
            .iter()
            .find(|pin| pin.name == name)
            .map(|pin| pin.bool_default)
            .unwrap_or_default();
        optional_data_back(ctx, name)?.map_or(Ok(fallback), |value| value.as_bool())
    }

    fn global_f32(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> f32 {
        if pin.global_key.is_empty() {
            return pin.f32_default;
        }
        crate::extensions::source_instance::source_input_data(ctx, &pin.global_key)
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(pin.f32_default)
    }

    fn global_bool(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> bool {
        if pin.global_key.is_empty() {
            return pin.bool_default;
        }
        crate::extensions::source_instance::source_input_data(ctx, &pin.global_key)
            .and_then(|value| value.as_bool().ok())
            .unwrap_or(pin.bool_default)
    }

    fn result_f32(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> Result<f32, GraphError> {
        match self.operation.as_str() {
            "absolute" => self.input_f32(ctx, "A").map(f32::abs),
            "bool_to_f32" => self
                .first_bool_input_name()
                .map_or(Ok(0.0), |name| self.input_bool(ctx, name).map(bool_to_f32)),
            "passthrough" => self
                .input_pins
                .first()
                .map_or(Ok(0.0), |input| self.input_f32(ctx, &input.name)),
            _ => Ok(self.global_f32(ctx, pin)),
        }
    }

    fn result_bool(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> Result<bool, GraphError> {
        let left = || self.input_f32(ctx, "A");
        let right = || self.input_f32(ctx, "B");
        Ok(match self.operation.as_str() {
            "less" => left()? < right()?,
            "less_equal" => left()? <= right()?,
            "greater" => left()? > right()?,
            "greater_equal" => left()? >= right()?,
            "equal" => {
                if let Some((first, second)) = self.bool_input_pair(ctx)? {
                    first == second
                } else {
                    (left()? - right()?).abs() <= f32::EPSILON
                }
            }
            "not_equal" => {
                if let Some((first, second)) = self.bool_input_pair(ctx)? {
                    first != second
                } else {
                    (left()? - right()?).abs() > f32::EPSILON
                }
            }
            "and" => self
                .input_pins
                .iter()
                .filter(|pin| pin.data_type == SourceScalarDataType::Bool)
                .try_fold(true, |value, pin| {
                    self.input_bool(ctx, &pin.name).map(|input| value & input)
                })?,
            "or" => self
                .input_pins
                .iter()
                .filter(|pin| pin.data_type == SourceScalarDataType::Bool)
                .try_fold(false, |value, pin| {
                    self.input_bool(ctx, &pin.name).map(|input| value | input)
                })?,
            "not" => !self.input_bool(ctx, "A")?,
            "gameplay_tag_valid" => self.input_bool(ctx, "GameplayTag")?,
            "passthrough" => self
                .input_pins
                .first()
                .map_or(Ok(false), |input| self.input_bool(ctx, &input.name))?,
            _ => self.global_bool(ctx, pin),
        })
    }
}

impl NodeLike for SourceScalarExpressionNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        for pin in &self.output_pins {
            match pin.data_type {
                SourceScalarDataType::F32 => {
                    ctx.set_data_fwd(pin.name.clone(), self.result_f32(&ctx, pin)?);
                }
                SourceScalarDataType::Bool => {
                    ctx.set_data_fwd(pin.name.clone(), self.result_bool(&ctx, pin)?);
                }
            }
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for pin in &self.input_pins {
            match pin.data_type {
                SourceScalarDataType::F32 => {
                    ctx.add_input_data(pin.name.clone(), DataSpec::F32);
                }
                SourceScalarDataType::Bool => {
                    ctx.add_input_data(pin.name.clone(), DataSpec::Bool);
                }
            }
        }
        for pin in &self.output_pins {
            match pin.data_type {
                SourceScalarDataType::F32 => {
                    ctx.add_output_data(pin.name.clone(), DataSpec::F32);
                }
                SourceScalarDataType::Bool => {
                    ctx.add_output_data(pin.name.clone(), DataSpec::Bool);
                }
            }
        }
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Scalar Expression".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reroute_passthrough_reads_declared_float_and_bool_input_pins() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::DataValue,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };

        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> DataValue {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            let context = GraphContext::new(
                arena.get_toplevel_id(),
                graph,
                &mut arena,
                &resources,
                &env,
                Entity::PLACEHOLDER,
                &entities,
                &mut gizmos,
                &inputs,
            );
            graph
                .query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context)
                .unwrap()["Result"]
                .clone()
        }

        let mut world = World::new();
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let mut failures = Vec::new();

        for (data_type, value, spec) in [
            (
                SourceScalarDataType::F32,
                DataValue::F32(2.5),
                DataSpec::F32,
            ),
            (
                SourceScalarDataType::Bool,
                DataValue::Bool(true),
                DataSpec::Bool,
            ),
        ] {
            let mut graph = AnimationGraph::new();
            let node = AnimationNode::new(
                "reroute",
                SourceScalarExpressionNode {
                    operation: "passthrough".into(),
                    input_pins: vec![SourceScalarPin {
                        name: "InputPin".into(),
                        data_type: data_type.clone(),
                        ..default()
                    }],
                    output_pins: vec![SourceScalarPin {
                        name: "OutputPin".into(),
                        data_type,
                        ..default()
                    }],
                    ..default()
                },
            );
            graph.add_node(node.clone());
            graph.set_default_data(GraphInputPin::passthrough("source"), value.clone());
            graph.add_input_data_edge(GraphInputPin::passthrough("source"), node.id, "InputPin");
            graph.add_output_data("Result".into(), spec);
            graph.add_output_data_edge(node.id, "OutputPin", "Result");
            let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            let actual = world.run_system_cached_with(query, handle).unwrap();
            match value {
                DataValue::F32(expected) if actual.as_f32().unwrap() != expected => {
                    failures.push(format!("float: {actual:?} != {expected}"));
                }
                DataValue::Bool(expected) if actual.as_bool().unwrap() != expected => {
                    failures.push(format!("bool: {actual:?} != {expected}"));
                }
                DataValue::F32(_) | DataValue::Bool(_) => {}
                _ => unreachable!(),
            }
        }
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn boolean_operators_use_every_declared_operand() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> bool {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            let context = GraphContext::new(
                arena.get_toplevel_id(),
                graph,
                &mut arena,
                &resources,
                &env,
                Entity::PLACEHOLDER,
                &entities,
                &mut gizmos,
                &inputs,
            );
            graph
                .query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context)
                .unwrap()["Result"]
                .as_bool()
                .unwrap()
        }
        let mut world = World::new();
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let mut failures = Vec::new();
        for count in 2..=4 {
            for bits in 0..(1 << count) {
                for operation in ["and", "or"] {
                    let mut graph = AnimationGraph::new();
                    let node = AnimationNode::new(
                        "boolean",
                        SourceScalarExpressionNode {
                            operation: operation.into(),
                            input_pins: (0..count)
                                .map(|index| SourceScalarPin {
                                    name: ((b'A' + index) as char).to_string(),
                                    data_type: SourceScalarDataType::Bool,
                                    bool_default: bits & (1 << index) != 0,
                                    ..default()
                                })
                                .collect(),
                            output_pins: vec![SourceScalarPin {
                                name: "ReturnValue".into(),
                                data_type: SourceScalarDataType::Bool,
                                ..default()
                            }],
                            ..default()
                        },
                    );
                    graph.add_node(node.clone());
                    graph.add_output_data("Result".into(), DataSpec::Bool);
                    graph.add_output_data_edge(node.id, "ReturnValue", "Result");
                    let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                    let actual = world.run_system_cached_with(query, handle).unwrap();
                    let expected = if operation == "and" {
                        bits == (1 << count) - 1
                    } else {
                        bits != 0
                    };
                    if actual != expected {
                        failures.push((operation, count, bits));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn scalar_expression_preserves_declared_pin_names_and_types() {
        let node = SourceScalarExpressionNode {
            input_pins: vec![
                SourceScalarPin {
                    name: "A".to_owned(),
                    data_type: SourceScalarDataType::F32,
                    f32_default: 1.0,
                    ..default()
                },
                SourceScalarPin {
                    name: "B".to_owned(),
                    data_type: SourceScalarDataType::F32,
                    f32_default: 2.0,
                    ..default()
                },
            ],
            output_pins: vec![SourceScalarPin {
                name: "Return Value".to_owned(),
                data_type: SourceScalarDataType::Bool,
                ..default()
            }],
            operation: "less".to_owned(),
            source_class: "K2Node_PromotableOperator".to_owned(),
            source_properties: vec!["Operation=Less".to_owned()],
        };
        assert_eq!(node.input_pins[0].name, "A");
        assert_eq!(node.input_pins[1].name, "B");
        assert_eq!(node.output_pins[0].name, "Return Value");
        assert_eq!(node.output_pins[0].data_type, SourceScalarDataType::Bool);
        assert_eq!(node.source_class, "K2Node_PromotableOperator");
    }

    #[test]
    fn scalar_expression_converts_boolean_to_source_float() {
        assert_eq!(bool_to_f32(false), 0.0);
        assert_eq!(bool_to_f32(true), 1.0);
    }

    #[test]
    fn boolean_to_float_uses_the_source_boolean_pin_name() {
        let node = SourceScalarExpressionNode {
            input_pins: vec![SourceScalarPin {
                name: "InBool".to_owned(),
                data_type: SourceScalarDataType::Bool,
                ..default()
            }],
            ..default()
        };
        assert_eq!(node.first_bool_input_name(), Some("InBool"));
    }
}
