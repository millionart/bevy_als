use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

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

    fn input_f32(&self, ctx: &NodeContext, name: &str) -> f32 {
        let fallback = self
            .input_pins
            .iter()
            .find(|pin| pin.name == name)
            .map(|pin| pin.f32_default)
            .unwrap_or_default();
        ctx.data_back(name.to_owned())
            .and_then(|value| value.as_f32())
            .unwrap_or(fallback)
    }

    fn input_bool(&self, ctx: &NodeContext, name: &str) -> bool {
        let fallback = self
            .input_pins
            .iter()
            .find(|pin| pin.name == name)
            .map(|pin| pin.bool_default)
            .unwrap_or_default();
        ctx.data_back(name.to_owned())
            .and_then(|value| value.as_bool())
            .unwrap_or(fallback)
    }

    fn global_f32(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> f32 {
        if pin.global_key.is_empty() {
            return pin.f32_default;
        }
        ctx.graph_context
            .global_input_data
            .get(&pin.global_key)
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(pin.f32_default)
    }

    fn global_bool(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> bool {
        if pin.global_key.is_empty() {
            return pin.bool_default;
        }
        ctx.graph_context
            .global_input_data
            .get(&pin.global_key)
            .and_then(|value| value.as_bool().ok())
            .unwrap_or(pin.bool_default)
    }

    fn result_f32(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> f32 {
        match self.operation.as_str() {
            "absolute" => self.input_f32(ctx, "A").abs(),
            "bool_to_f32" => bool_to_f32(
                self.first_bool_input_name()
                    .map(|name| self.input_bool(ctx, name))
                    .unwrap_or(false),
            ),
            "passthrough" => self.input_f32(ctx, "A"),
            _ => self.global_f32(ctx, pin),
        }
    }

    fn result_bool(&self, ctx: &NodeContext, pin: &SourceScalarPin) -> bool {
        let left = self.input_f32(ctx, "A");
        let right = self.input_f32(ctx, "B");
        match self.operation.as_str() {
            "less" => left < right,
            "less_equal" => left <= right,
            "greater" => left > right,
            "greater_equal" => left >= right,
            "equal" => {
                let bool_inputs = self
                    .input_pins
                    .iter()
                    .filter(|value| value.data_type == SourceScalarDataType::Bool)
                    .collect::<Vec<_>>();
                if bool_inputs.len() >= 2 {
                    self.input_bool(ctx, &bool_inputs[0].name)
                        == self.input_bool(ctx, &bool_inputs[1].name)
                } else {
                    (left - right).abs() <= f32::EPSILON
                }
            }
            "not_equal" => (left - right).abs() > f32::EPSILON,
            "and" => self.input_bool(ctx, "A") && self.input_bool(ctx, "B"),
            "or" => self.input_bool(ctx, "A") || self.input_bool(ctx, "B"),
            "not" => !self.input_bool(ctx, "A"),
            "passthrough" => self.input_bool(ctx, "A"),
            _ => self.global_bool(ctx, pin),
        }
    }
}

impl NodeLike for SourceScalarExpressionNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        for pin in &self.output_pins {
            match pin.data_type {
                SourceScalarDataType::F32 => {
                    ctx.set_data_fwd(pin.name.clone(), self.result_f32(&ctx, pin));
                }
                SourceScalarDataType::Bool => {
                    ctx.set_data_fwd(pin.name.clone(), self.result_bool(&ctx, pin));
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
