use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use serde::{Deserialize, Serialize};

pub(crate) const WEIGHT_THRESHOLD: f32 = 1e-6;

pub const ACTIVE_INDEX: &str = "active_index";
pub const UPSTREAM_TIME: &str = "upstream_time";

pub fn weight_output(index: usize) -> String {
    format!("weight_{index}")
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum GameplayTagBlendMode {
    #[default]
    Linear,
    HermiteCubic,
}

impl GameplayTagBlendMode {
    fn apply(self, alpha: f32) -> f32 {
        let alpha = alpha.clamp(0.0, 1.0);
        match self {
            Self::Linear => alpha,
            Self::HermiteCubic => alpha * alpha * (3.0 - 2.0 * alpha),
        }
    }
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct GameplayTagBlendState {
    initialized: bool,
    last_active_index: i32,
    start_weights: Vec<f32>,
    weights: Vec<f32>,
    elapsed: f32,
    duration: f32,
}

pub(crate) fn select_gameplay_tag_child_index(tags: &[String], value: f32) -> usize {
    if !value.is_finite() || value < 0.0 {
        return 0;
    }
    let index = value.round() as usize;
    if index <= tags.len() { index } else { 0 }
}

pub(crate) fn gameplay_tag_blend_time(blend_times: &[f32], index: usize) -> f32 {
    blend_times.get(index).copied().unwrap_or_default().max(0.0)
}

pub(crate) fn advance_gameplay_tag_weights(
    state: &mut GameplayTagBlendState,
    child_count: usize,
    active_index: usize,
    blend_time: f32,
    blend_mode: GameplayTagBlendMode,
    delta_seconds: f32,
) -> Vec<f32> {
    if !state.initialized || state.weights.len() != child_count {
        state.initialized = true;
        state.last_active_index = -1;
        state.start_weights = vec![0.0; child_count];
        state.weights = vec![0.0; child_count];
        state.weights[0] = 1.0;
        state.start_weights.clone_from(&state.weights);
        state.elapsed = 0.0;
        state.duration = 0.0;
    }

    if state.last_active_index != active_index as i32 {
        let current_weight = state.weights[active_index];
        state.start_weights.clone_from(&state.weights);
        state.elapsed = 0.0;
        state.duration = if state.last_active_index < 0 {
            0.0
        } else {
            blend_time * (1.0 - current_weight).abs().clamp(0.0, 1.0)
        };
        state.last_active_index = active_index as i32;
    }

    if state.duration <= WEIGHT_THRESHOLD {
        state.weights.fill(0.0);
        state.weights[active_index] = 1.0;
    } else {
        state.elapsed = (state.elapsed + delta_seconds.max(0.0)).min(state.duration);
        let alpha = blend_mode.apply(state.elapsed / state.duration);
        for (index, weight) in state.weights.iter_mut().enumerate() {
            let target = if index == active_index { 1.0 } else { 0.0 };
            *weight = state.start_weights[index] + (target - state.start_weights[index]) * alpha;
        }
        let sum = state.weights.iter().sum::<f32>();
        if sum > WEIGHT_THRESHOLD && (sum - 1.0).abs() > WEIGHT_THRESHOLD {
            for weight in &mut state.weights {
                *weight /= sum;
            }
        }
    }

    state.weights.clone()
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GameplayTagBlendNode {
    pub tags: Vec<String>,
    pub blend_times: Vec<f32>,
    pub blend_mode: GameplayTagBlendMode,
    pub forward_time: bool,
}

impl Default for GameplayTagBlendNode {
    fn default() -> Self {
        Self {
            tags: default(),
            blend_times: vec![0.0],
            blend_mode: default(),
            forward_time: true,
        }
    }
}

impl GameplayTagBlendNode {
    fn child_count(&self) -> usize {
        self.tags.len() + 1
    }

    fn select_child_index(&self, value: f32) -> usize {
        select_gameplay_tag_child_index(&self.tags, value)
    }

    fn blend_time(&self, index: usize) -> f32 {
        gameplay_tag_blend_time(&self.blend_times, index)
    }

    fn advance_weights(
        state: &mut GameplayTagBlendState,
        child_count: usize,
        active_index: usize,
        blend_time: f32,
        blend_mode: GameplayTagBlendMode,
        delta_seconds: f32,
    ) -> Vec<f32> {
        advance_gameplay_tag_weights(
            state,
            child_count,
            active_index,
            blend_time,
            blend_mode,
            delta_seconds,
        )
    }
}

impl NodeLike for GameplayTagBlendNode {
    fn display_name(&self) -> String {
        "ALS Gameplay Tag Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.forward_time {
            ctx.set_duration_fwd(ctx.duration_back(UPSTREAM_TIME)?);
        } else {
            ctx.set_duration_fwd(None);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let active_value = ctx.data_back(ACTIVE_INDEX)?.as_f32()?;
        let time_update = ctx.time_update_fwd()?;
        let delta_seconds = match &time_update {
            TimeUpdate::Delta(value) => *value,
            TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
        };
        let active_index = self.select_child_index(active_value);
        let child_count = self.child_count();
        let weights = {
            let state = ctx.state_mut::<GameplayTagBlendState>()?;
            Self::advance_weights(
                state,
                child_count,
                active_index,
                self.blend_time(active_index),
                self.blend_mode,
                delta_seconds,
            )
        };
        if self.forward_time {
            ctx.set_time_update_back(UPSTREAM_TIME, time_update.clone());
        }
        let time = time_update
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.time());
        ctx.set_time(time);
        for (index, weight) in weights.iter().copied().enumerate() {
            ctx.set_data_fwd(weight_output(index), weight);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(ACTIVE_INDEX, DataSpec::F32)
            .add_input_time(UPSTREAM_TIME)
            .add_output_time();
        for index in 0..self.child_count() {
            ctx.add_output_data(weight_output(index), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_uses_default_for_invalid_indices() {
        let node = GameplayTagBlendNode {
            tags: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
            blend_times: vec![0.3, 0.1, 0.0, 0.1],
            blend_mode: GameplayTagBlendMode::Linear,
            forward_time: true,
        };
        assert_eq!(node.select_child_index(-1.0), 0);
        assert_eq!(node.select_child_index(2.0), 2);
        assert_eq!(node.select_child_index(4.0), 0);
        assert_eq!(node.select_child_index(f32::NAN), 0);
    }

    #[test]
    fn default_forwards_upstream_time() {
        assert!(GameplayTagBlendNode::default().forward_time);
    }

    #[test]
    fn weights_follow_target_child_blend_time() {
        let mut state = GameplayTagBlendState::default();
        assert_eq!(
            GameplayTagBlendNode::advance_weights(
                &mut state,
                4,
                0,
                0.3,
                GameplayTagBlendMode::Linear,
                0.0,
            ),
            vec![1.0, 0.0, 0.0, 0.0]
        );
        let middle = GameplayTagBlendNode::advance_weights(
            &mut state,
            4,
            3,
            0.1,
            GameplayTagBlendMode::Linear,
            0.05,
        );
        assert!((middle[0] - 0.5).abs() < 1e-6);
        assert!((middle[3] - 0.5).abs() < 1e-6);
        assert_eq!(
            GameplayTagBlendNode::advance_weights(
                &mut state,
                4,
                3,
                0.1,
                GameplayTagBlendMode::Linear,
                0.05,
            ),
            vec![0.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn zero_blend_time_switches_immediately() {
        let mut state = GameplayTagBlendState::default();
        GameplayTagBlendNode::advance_weights(
            &mut state,
            4,
            0,
            0.3,
            GameplayTagBlendMode::Linear,
            0.0,
        );
        assert_eq!(
            GameplayTagBlendNode::advance_weights(
                &mut state,
                4,
                2,
                0.0,
                GameplayTagBlendMode::Linear,
                0.01,
            ),
            vec![0.0, 0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn interrupted_blend_scales_the_new_target_duration() {
        let mut state = GameplayTagBlendState::default();
        GameplayTagBlendNode::advance_weights(
            &mut state,
            2,
            0,
            0.3,
            GameplayTagBlendMode::Linear,
            0.0,
        );
        let halfway = GameplayTagBlendNode::advance_weights(
            &mut state,
            2,
            1,
            0.2,
            GameplayTagBlendMode::Linear,
            0.1,
        );
        assert!((halfway[0] - 0.5).abs() < 1e-6);
        assert!((halfway[1] - 0.5).abs() < 1e-6);
        let retargeted = GameplayTagBlendNode::advance_weights(
            &mut state,
            2,
            0,
            0.3,
            GameplayTagBlendMode::Linear,
            0.075,
        );
        assert!((retargeted[0] - 0.75).abs() < 1e-6);
        assert!((retargeted[1] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn hermite_cubic_uses_smooth_step_weight_curve() {
        let mut state = GameplayTagBlendState::default();
        GameplayTagBlendNode::advance_weights(
            &mut state,
            2,
            0,
            0.0,
            GameplayTagBlendMode::HermiteCubic,
            0.0,
        );
        let weights = GameplayTagBlendNode::advance_weights(
            &mut state,
            2,
            1,
            1.0,
            GameplayTagBlendMode::HermiteCubic,
            0.25,
        );
        assert!((weights[0] - 0.84375).abs() < 1e-6);
        assert!((weights[1] - 0.15625).abs() < 1e-6);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<GameplayTagBlendNode>();
        let registration = registry
            .get(std::any::TypeId::of::<GameplayTagBlendNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
