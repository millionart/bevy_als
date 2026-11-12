use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use bevy_gameplay_tag::GameplayTag;
use serde::{Deserialize, Serialize};

use crate::extensions::source_gameplay_tag_input::active_gameplay_tag_input;
use crate::extensions::profile_blend::{AlphaBlendMode, advance_alpha_blend, transition_alpha};

pub(crate) const WEIGHT_THRESHOLD: f32 = 1e-6;

pub const UPSTREAM_TIME: &str = "upstream_time";

pub fn weight_output(index: usize) -> String {
    format!("weight_{index}")
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum GameplayTagBlendMode {
    #[default]
    Linear,
    Cubic,
    HermiteCubic,
}

impl GameplayTagBlendMode {
    fn apply(self, alpha: f32) -> f32 {
        let mode = match self {
            Self::Linear => AlphaBlendMode::Linear,
            Self::Cubic => AlphaBlendMode::Cubic,
            Self::HermiteCubic => AlphaBlendMode::HermiteCubic,
        };
        transition_alpha(alpha, mode, &[])
    }
}

#[derive(Reflect, Clone, Debug, Default)]
struct BlendListChildState {
    begin: f32,
    alpha: f32,
    remaining: f32,
    value: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct GameplayTagBlendState {
    initialized: bool,
    last_active_index: i32,
    clocks: Vec<BlendListChildState>,
    weights: Vec<f32>,
}

impl GameplayTagBlendState {
    pub(crate) fn should_reset_child(&self, child_count: usize, active_index: usize) -> bool {
        let initialized = self.initialized && self.weights.len() == child_count;
        let weight = if initialized { self.weights[active_index] } else { f32::from(active_index == 0) };
        (!initialized || self.last_active_index != active_index as i32) && weight <= 0.00001
    }

    pub(crate) fn instant_transition_source(&self, active_index: usize, blend_time: f32) -> Option<usize> {
        (self.initialized && self.last_active_index >= 0
            && self.last_active_index != active_index as i32
            && blend_time.max(0.0) * (1.0 - self.weights[active_index]).abs().clamp(0.0, 1.0) == 0.0)
            .then_some(self.last_active_index as usize)
    }
}

pub(crate) fn select_gameplay_tag_child_index_for_tag(
    tags: &[String],
    active_tag: Option<&GameplayTag>,
) -> usize {
    active_tag
        .filter(|tag| tag.is_valid())
        .and_then(|tag| {
            tags.iter()
                .position(|candidate| tag.as_str().eq_ignore_ascii_case(GameplayTag::new(candidate).as_str()))
        })
        .map_or(0, |index| index + 1)
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
    advance_blend_list_weights(
        state,
        child_count,
        active_index,
        blend_time,
        delta_seconds,
        |alpha| blend_mode.apply(alpha),
    )
}

pub(crate) fn advance_blend_list_weights(
    state: &mut GameplayTagBlendState,
    child_count: usize,
    active_index: usize,
    blend_time: f32,
    delta_seconds: f32,
    blend_alpha: impl Fn(f32) -> f32,
) -> Vec<f32> {
    if !state.initialized || state.weights.len() != child_count {
        state.initialized = true;
        state.last_active_index = -1;
        state.clocks = vec![BlendListChildState::default(); child_count];
        state.clocks[0].value = 1.0;
        state.weights = vec![0.0; child_count];
        state.weights[0] = 1.0;
    }

    if state.last_active_index != active_index as i32 {
        let current_weight = state.weights[active_index];
        let duration = if state.last_active_index < 0 {
            0.0
        } else {
            blend_time.max(0.0) * (1.0 - current_weight).abs().clamp(0.0, 1.0)
        };
        for (index, (clock, weight)) in state.clocks.iter_mut().zip(&state.weights).enumerate() {
            clock.begin = *weight;
            let target = f32::from(index == active_index);
            clock.alpha = if duration <= 0.0 || clock.begin == target {
                1.0
            } else {
                ((clock.value - clock.begin) / (target - clock.begin)).clamp(0.0, 1.0)
            };
            clock.remaining = duration * (1.0 - clock.alpha);
            clock.value = clock.begin + (target - clock.begin) * blend_alpha(clock.alpha);
        }
        state.last_active_index = active_index as i32;
    }

    let terminal_alpha = blend_alpha(1.0);
    let delta = delta_seconds.max(0.0);
    for (index, (clock, weight)) in state.clocks.iter_mut().zip(&mut state.weights).enumerate() {
        let range = f32::from(index == active_index) - clock.begin;
        if clock.value != clock.begin + range * terminal_alpha {
            advance_alpha_blend(&mut clock.alpha, &mut clock.remaining, delta);
            clock.value = clock.begin + range * blend_alpha(clock.alpha);
        }
        *weight = clock.value;
    }
    let sum = state.weights.iter().sum::<f32>();
    if sum > 1e-5 && (sum - 1.0).abs() > 1e-5 {
        let reciprocal = 1.0 / sum;
        for weight in &mut state.weights {
            *weight *= reciprocal;
        }
    }

    state.weights.clone()
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GameplayTagBlendNode {
    pub active_key: String,
    pub tags: Vec<String>,
    pub blend_times: Vec<f32>,
    pub blend_mode: GameplayTagBlendMode,
    pub forward_time: bool,
}

impl Default for GameplayTagBlendNode {
    fn default() -> Self {
        Self {
            active_key: default(),
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

    fn select_child_index(&self, tag: &GameplayTag) -> usize {
        select_gameplay_tag_child_index_for_tag(&self.tags, Some(tag))
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
        let active_tag = active_gameplay_tag_input(ctx.graph_context.root_entity, &self.active_key)
            .ok_or_else(|| GraphError::GlobalInputDataMissing(self.active_key.clone()))?;
        let time_update = ctx.time_update_fwd()?;
        let delta_seconds = match &time_update {
            TimeUpdate::Delta(value) => *value,
            TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
        };
        let active_index = self.select_child_index(&active_tag);
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
        ctx.add_input_time(UPSTREAM_TIME).add_output_time();
        for index in 0..self.child_count() {
            ctx.add_output_data(weight_output(index), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn browser_review_tag_blend_selects_case_alias_with_exact_source_semantics() {
        let tags=vec!["Als.OverlayMode.Rifle".to_owned()];
        assert_eq!(select_gameplay_tag_child_index_for_tag(&tags,Some(&GameplayTag::new("als.overlaymode.rifle"))),1);
        assert_eq!(select_gameplay_tag_child_index_for_tag(&tags,Some(&GameplayTag::new("Als.OverlayMode.Rifle.Child"))),0);
        assert_eq!(select_gameplay_tag_child_index_for_tag(&tags,Some(&GameplayTag::new(""))),0);
    }

    use super::*;

    #[test]
    fn blend_list_weights_match_native_cubic_reversals_and_hermite_children() {
        #[derive(Deserialize)]
        struct Frame {
            duration: f32,
            weights: Vec<f32>,
            active: usize,
            delta: f32,
        }
        #[derive(Deserialize)]
        struct Case {
            name: String,
            frames: Vec<Frame>,
        }
        #[derive(Deserialize)]
        struct Probe {
            mode: GameplayTagBlendMode,
            times: Vec<f32>,
            cases: Vec<Case>,
        }
        #[derive(Deserialize)]
        struct Reference {
            probes: Vec<Probe>,
        }

        let reference: Reference = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d214/weights.replay.json"
        )).unwrap();
        for probe in reference.probes {
            for case in probe.cases {
                let mut state = GameplayTagBlendState::default();
                for (index, frame) in case.frames.into_iter().enumerate() {
                    let changed = !state.initialized || state.last_active_index != frame.active as i32;
                    let weights = advance_gameplay_tag_weights(
                        &mut state, probe.times.len(), frame.active, probe.times[frame.active],
                        probe.mode, frame.delta,
                    );
                    assert_eq!(weights, frame.weights, "{} at frame {index}", case.name);
                    let active = &state.clocks[frame.active];
                    if changed && frame.delta == 0.0 && active.alpha == 0.0 {
                        assert_eq!(active.remaining, frame.duration, "{} at frame {index}", case.name);
                    }
                }
            }
        }
    }

    #[test]
    fn child_reset_matches_source_active_index_and_weight_boundary() {
        let mut state = GameplayTagBlendState::default();
        assert!(!state.should_reset_child(2, 0));
        assert!(state.should_reset_child(2, 1));
        advance_gameplay_tag_weights(&mut state, 2, 0, 0.5, GameplayTagBlendMode::Linear, 0.1);
        assert!(state.should_reset_child(2, 1));
        advance_gameplay_tag_weights(&mut state, 2, 1, 0.5, GameplayTagBlendMode::Linear, 0.25);
        assert!(!state.should_reset_child(2, 0));
        for (weight, reset) in [(0.0, true), (0.00001, true), (0.0000101, false)] {
            state.weights = vec![weight, 1.0 - weight];
            assert_eq!(state.should_reset_child(2, 0), reset);
        }
        state.last_active_index = 0;
        state.weights[0] = 0.0;
        assert!(!state.should_reset_child(2, 0));
    }

    #[test]
    fn selection_uses_default_for_invalid_or_unknown_tags() {
        let node = GameplayTagBlendNode {
            active_key: "locomotion_action".to_owned(),
            tags: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
            blend_times: vec![0.3, 0.1, 0.0, 0.1],
            blend_mode: GameplayTagBlendMode::Linear,
            forward_time: true,
        };
        assert_eq!(node.select_child_index(&GameplayTag::new("B")), 2);
        assert_eq!(node.select_child_index(&GameplayTag::new("Missing")), 0);
        assert_eq!(node.select_child_index(&GameplayTag::new("")), 0);
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
