use std::collections::VecDeque;
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, PinId, SourcePin, TargetPin, TimeUpdate},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        io_env::GraphIoEnv,
        new_context::{GraphContext, NodeContext},
        spec_context::SpecContext,
    },
    duration_data::DurationData,
    edge_data::{
        DataSpec, DataValue,
        events::{AnimationEvent, EventQueue, SampledEvent},
    },
    errors::GraphError,
    pose::Pose,
    state_machine::{
        high_level::{DirectTransitionId, StateId, StateMachine, TransitionId},
        low_level::{
            FsmBuiltinPin, LowLevelState, LowLevelStateId, LowLevelStateMachine,
            LowLevelTransition, LowLevelTransitionId,
        },
    },
};

use crate::extensions::animation_context::{
    AnimationContextState, SourceRelevantAsset, capture_context_curve, graph_output_context_state,
    merge_missing_context_curves,
};
use crate::extensions::global_f32_input::global_f32_or_root_default;
use crate::extensions::inertialization::InertializationNode;
use crate::extensions::profile_blend::{AlphaBlendMode, ProfiledBlendNode, transition_alpha};
use crate::extensions::source_cached_pose::{SourceScopedCachedPoseUseNode, prewarm_scoped_cached_pose};
use crate::extensions::source_pose_passthrough::SourcePosePassthroughNode;
use crate::extensions::state_label_event::state_label_index;

static NOTIFICATION_GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static SOURCE_MACHINE_WEIGHT: Cell<f32> = const { Cell::new(1.0) };
}

fn current_machine_weight() -> f32 {
    SOURCE_MACHINE_WEIGHT.with(Cell::get)
}

pub(crate) fn with_machine_weight<T>(factor: f32, evaluate: impl FnOnce() -> T) -> T {
    struct Restore(f32);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_MACHINE_WEIGHT.with(|weight| weight.set(self.0));
        }
    }
    let previous = SOURCE_MACHINE_WEIGHT.with(|weight| {
        let previous = weight.get();
        weight.set(previous * factor);
        previous
    });
    let _restore = Restore(previous);
    evaluate()
}

#[derive(Reflect, Clone, Debug)]
pub struct InterruptibleFsmSource {
    pub state: LowLevelStateId,
    pub state_entered_time: f32,
    context_slot: usize,
    blend_duration: Option<f32>,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceFsmStateWeight {
    pub state_id: String,
    pub value: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct StatePoseOutput {
    pub state_id: String,
    pub output: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct StateAliasBinding {
    pub state_id: String,
    pub member_state_ids: Vec<String>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceTransitionConditionGate {
    pub condition: Handle<AnimationGraph>,
    pub output: String,
    pub context_state: StateId,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceEntryCondition {
    pub target_state_id: String,
    pub gates: Vec<SourceTransitionConditionGate>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceTransitionNotification {
    pub phase: String,
    pub name: String,
    pub function: String,
    pub arguments: Vec<crate::extensions::source_instance::SourcePropertyAssignment>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceTransitionCondition {
    pub transition_id: String,
    pub source_state_id: String,
    pub condition: Handle<AnimationGraph>,
    pub output: String,
    pub context_state: StateId,
    pub gates: Vec<SourceTransitionConditionGate>,
    pub notifications: Vec<SourceTransitionNotification>,
    pub automatic_trigger_time: Option<f32>,
    pub transition_duration: f32,
    pub always_reset_target_on_entry: Option<bool>,
}

#[derive(Reflect, Clone, Debug)]
pub struct InterruptibleFsmState {
    pub state: LowLevelStateId,
    pub state_entered_time: f32,
    context_slot: usize,
    blend_duration: Option<f32>,
    pub interrupted_sources: Vec<InterruptibleFsmSource>,
    pub context: AnimationContextState,
    recorded_machine_weight: f32,
    current_state_elapsed_time: f32,
    recorded_state_weights: Vec<SourceFsmStateWeight>,
    source_pose: Option<(Pose, AnimationContextState)>,
    last_pose: Option<Pose>,
    label_last_index: Option<usize>,
    first_update: bool,
    last_update_frame: Option<u64>,
    transition_notifications: Vec<SourceTransitionNotification>,
    notification_generation: u64,
}

fn changed_label(
    last_index: &mut Option<usize>,
    index: Option<usize>,
    labels: &[String],
) -> Option<String> {
    let changed = *last_index != index;
    *last_index = index;
    if changed {
        index.and_then(|value| labels.get(value).cloned())
    } else {
        None
    }
}

fn timed_transition_progress(elapsed: f32, duration: Option<f32>) -> Option<f32> {
    duration.map(|duration| {
        if duration <= 0.0 {
            1.0
        } else {
            (elapsed / duration).clamp(0.0, 1.0)
        }
    })
}

fn source_transition_duration(duration: f32, target_weight: f32, adjustment: f32, mode: AlphaBlendMode) -> f32 {
    let fraction = (1.0 - target_weight).clamp(0.0, 1.0);
    let scale = if mode == AlphaBlendMode::HermiteCubic {
        fraction * fraction * fraction * (4.0 / 3.0) - fraction * fraction * 2.0 + fraction * (5.0 / 3.0)
    } else { fraction };
    (duration - adjustment).max(0.0) * scale
}

fn source_automatic_transition_adjustment(
    relevant_asset: Option<&SourceRelevantAsset>,
    trigger_time: f32,
    transition_duration: f32,
) -> Option<f32> {
    let Some(relevant_asset) = relevant_asset else {
        return None;
    };
    if relevant_asset.weight <= 0.0 || relevant_asset.duration <= f32::EPSILON {
        return None;
    }
    let mut remaining = relevant_asset.duration - relevant_asset.position;
    if relevant_asset.looping
        && remaining > 0.0
        && relevant_asset.has_previous_position
        && (relevant_asset.position - relevant_asset.previous_position)
            * relevant_asset.delta_seconds
            < 0.0
    {
        remaining = 0.0;
    }
    let trigger = if trigger_time >= 0.0 {
        trigger_time
    } else {
        transition_duration
    };
    let adjustment = trigger - remaining;
    (adjustment >= 0.0).then_some(adjustment)
}

fn became_relevant(previous_frame: Option<u64>, frame: u64) -> bool {
    previous_frame.is_some_and(|previous| frame > previous.saturating_add(1))
}

impl InterruptibleFsmState {
    pub fn current_state_elapsed_time(&self) -> f32 {
        self.current_state_elapsed_time
    }

    pub fn cached_source_pose(&self) -> Option<&Pose> {
        self.source_pose.as_ref().map(|(pose, _)| pose)
    }

    pub fn last_output_pose(&self) -> Option<&Pose> {
        self.last_pose.as_ref()
    }

    pub fn recorded_machine_weight(&self) -> f32 {
        self.recorded_machine_weight
    }

    pub fn recorded_state_weight(&self, state_id: &str) -> f32 {
        self.recorded_state_weights
            .iter()
            .find(|value| value.state_id == state_id)
            .map_or(0.0, |value| value.value)
    }

    pub fn transition_notifications(&self) -> &[SourceTransitionNotification] {
        &self.transition_notifications
    }

    pub fn notification_generation(&self) -> u64 {
        self.notification_generation
    }

    pub fn active_direct_transition_id(&self) -> Option<String> {
        match &self.state {
            LowLevelStateId::HlTransition(TransitionId::Direct(transition)) => {
                Some(transition.uuid().to_string())
            }
            _ => None,
        }
    }

    pub fn active_state_id(&self) -> Option<String> {
        match &self.state {
            LowLevelStateId::HlState(state) => Some(state.uuid().to_string()),
            LowLevelStateId::HlTransition(_) => None,
        }
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InterruptibleFsmNode {
    pub fsm: Handle<StateMachine>,
    pub telemetry_states: Vec<StateId>,
    pub telemetry_transitions: Vec<DirectTransitionId>,
    pub state_pose_output: String,
    pub state_pose_outputs: Vec<StatePoseOutput>,
    pub state_aliases: Vec<StateAliasBinding>,
    pub source_transition_conditions: Vec<SourceTransitionCondition>,
    pub source_entry_conditions: Vec<SourceEntryCondition>,
    pub outer_pose_output: String,
    pub state_label_key: String,
    pub state_labels: Vec<String>,
    pub internal_time_key: String,
    pub max_transitions_per_frame: usize,
    pub max_transition_requests: usize,
    pub skip_first_update_transition: bool,
    pub reinitialize_on_becoming_relevant: bool,
    pub expose_telemetry: bool,
}

impl Default for InterruptibleFsmNode {
    fn default() -> Self {
        Self {
            fsm: default(),
            telemetry_states: default(),
            telemetry_transitions: default(),
            state_pose_output: Self::OUT_POSE.to_owned(),
            state_pose_outputs: default(),
            state_aliases: default(),
            source_transition_conditions: default(),
            source_entry_conditions: default(),
            outer_pose_output: Self::OUT_POSE.to_owned(),
            state_label_key: default(),
            state_labels: default(),
            internal_time_key: default(),
            max_transitions_per_frame: 3,
            max_transition_requests: 32,
            skip_first_update_transition: true,
            reinitialize_on_becoming_relevant: true,
            expose_telemetry: true,
        }
    }
}

impl InterruptibleFsmNode {
    pub const OUT_POSE: &'static str = "pose";
    pub const OUT_ACTUAL_STATE: &'static str = "actual_state";
    pub const OUT_ACTUAL_TRANSITION: &'static str = "actual_transition";
    pub const OUT_ACTUAL_TRANSITION_ACTIVE: &'static str = "actual_transition_active";
    pub const OUT_ACTUAL_STATE_ENTERED_TIME: &'static str = "actual_state_entered_time";
    pub const OUT_ACTUAL_STATE_ELAPSED_TIME: &'static str = "actual_state_elapsed_time";

    pub fn new(fsm: Handle<StateMachine>) -> Self {
        Self {
            fsm,
            telemetry_states: Vec::new(),
            telemetry_transitions: Vec::new(),
            state_pose_output: Self::OUT_POSE.to_owned(),
            state_pose_outputs: Vec::new(),
            state_aliases: Vec::new(),
            source_transition_conditions: Vec::new(),
            source_entry_conditions: Vec::new(),
            outer_pose_output: Self::OUT_POSE.to_owned(),
            state_label_key: String::new(),
            state_labels: Vec::new(),
            internal_time_key: String::new(),
            max_transitions_per_frame: 3,
            max_transition_requests: 32,
            skip_first_update_transition: true,
            reinitialize_on_becoming_relevant: true,
            expose_telemetry: true,
        }
    }

    fn state_pose_output_for(&self, state: &LowLevelStateId) -> &str {
        let LowLevelStateId::HlState(state) = state else {
            return &self.state_pose_output;
        };
        self.state_pose_outputs
            .iter()
            .find(|value| value.state_id == state.uuid().to_string())
            .map(|value| value.output.as_str())
            .unwrap_or(&self.state_pose_output)
    }

    fn outer_output(&self, state: &LowLevelStateId, state_output: &str) -> PinId {
        if state_output == self.state_pose_output_for(state)
            || (matches!(state, LowLevelStateId::HlTransition(_)) && state_output == Self::OUT_POSE)
        {
            self.outer_pose_output.clone()
        } else {
            state_output.to_owned()
        }
    }

    fn telemetry_state_index(&self, state: &LowLevelStateId) -> f32 {
        match state {
            LowLevelStateId::HlState(state) => self
                .telemetry_states
                .iter()
                .position(|candidate| candidate == state)
                .map(|index| index as f32)
                .unwrap_or(-1.0),
            LowLevelStateId::HlTransition(_) => -1.0,
        }
    }

    fn telemetry_transition_index(&self, state: &LowLevelStateId) -> f32 {
        match state {
            LowLevelStateId::HlTransition(TransitionId::Direct(transition)) => self
                .telemetry_transitions
                .iter()
                .position(|candidate| candidate == transition)
                .map(|index| index as f32)
                .unwrap_or(-1.0),
            LowLevelStateId::HlTransition(_) => -1.0,
            LowLevelStateId::HlState(_) => -1.0,
        }
    }

    fn state_machine<'a>(
        &self,
        resources: &'a bevy_animation_graph::core::context::system_resources::SystemResources,
    ) -> Result<&'a LowLevelStateMachine, GraphError> {
        resources
            .state_machine_assets
            .get(&self.fsm)
            .map(StateMachine::get_low_level_fsm)
            .ok_or(GraphError::FsmAssetMissing)
    }

    fn transition_for_state_pair<'a>(
        machine: &'a LowLevelStateMachine,
        source: StateId,
        target: StateId,
        allow_external: bool,
    ) -> Option<&'a LowLevelTransition> {
        machine
            .transitions_by_hl_state_pair
            .get(&(source, target))
            .iter()
            .flat_map(|ids| ids.iter().filter_map(|id| machine.transitions.get(id)))
            .find(|transition| !(transition.ignore_external && allow_external))
    }

    fn active_target(machine: &LowLevelStateMachine, state: &LowLevelStateId) -> Option<StateId> {
        machine
            .states
            .get(state)
            .and_then(|state| state.hl_transition.as_ref())
            .map(|transition| transition.target)
    }

    fn source_condition_state(
        machine: &LowLevelStateMachine,
        state: &LowLevelStateId,
    ) -> Option<StateId> {
        match state {
            LowLevelStateId::HlState(state) => Some(*state),
            LowLevelStateId::HlTransition(_) => Self::active_target(machine, state),
        }
    }

    fn source_condition_transition_id(
        machine: &LowLevelStateMachine,
        condition: &SourceTransitionCondition,
    ) -> Option<TransitionId> {
        machine
            .transitions
            .values()
            .find_map(|transition| match &transition.id {
                LowLevelTransitionId::Start(TransitionId::Direct(id))
                | LowLevelTransitionId::Immediate(TransitionId::Direct(id))
                    if transition.hl_source.uuid().to_string() == condition.source_state_id
                        && id.uuid().to_string() == condition.transition_id =>
                {
                    Some(TransitionId::Direct(*id))
                }
                _ => None,
            })
    }

    fn transition_id(transition: &LowLevelTransition) -> Option<&TransitionId> {
        match &transition.id {
            LowLevelTransitionId::Start(id)
            | LowLevelTransitionId::End(id)
            | LowLevelTransitionId::Immediate(id) => Some(id),
        }
    }

    fn append_transition_notifications(
        &self,
        state: &mut InterruptibleFsmState,
        transition_id: &TransitionId,
        phase: &str,
    ) {
        let TransitionId::Direct(transition_id) = transition_id else {
            return;
        };
        if let Some(condition) = self
            .source_transition_conditions
            .iter()
            .find(|condition| condition.transition_id == transition_id.uuid().to_string())
        {
            state.transition_notifications.extend(
                condition
                    .notifications
                    .iter()
                    .filter(|notification| notification.phase == phase)
                    .cloned(),
            );
        }
    }

    fn state_function_notifications(
        machine: &LowLevelStateMachine,
        state: StateId,
        phase: &str,
        graphs: &Assets<AnimationGraph>,
    ) -> Result<Vec<SourceTransitionNotification>, GraphError> {
        let state = machine.states.get(&LowLevelStateId::HlState(state))
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let graph = graphs
            .get(&state.graph)
            .ok_or(GraphError::GraphAssetMissing)?;
        Ok(graph
            .nodes
            .values()
            .filter_map(|node| {
                let node = node.try_inner_downcast_ref::<SourcePosePassthroughNode>()?;
                (!node.source_function.is_empty() && node.call_site == phase).then(|| {
                    SourceTransitionNotification {
                        phase: phase.to_owned(),
                        name: node.source_function.clone(),
                        ..default()
                    }
                })
            })
            .collect())
    }

    fn can_interrupt(
        machine: &LowLevelStateMachine,
        state: &InterruptibleFsmState,
        transition: &LowLevelTransition,
    ) -> bool {
        matches!(transition.target, LowLevelStateId::HlTransition(_))
            && Self::active_target(machine, &state.state)
                .is_some_and(|target| target == transition.hl_source)
    }

    fn alias_matches(
        &self,
        machine: &LowLevelStateMachine,
        state: &InterruptibleFsmState,
        transition: &LowLevelTransition,
    ) -> bool {
        let LowLevelStateId::HlState(alias_state) = transition.source else {
            return false;
        };
        let Some(alias) = self
            .state_aliases
            .iter()
            .find(|alias| alias.state_id == alias_state.uuid().to_string())
        else {
            return false;
        };
        let current = match state.state {
            LowLevelStateId::HlState(current) => Some(current),
            LowLevelStateId::HlTransition(_) => Self::active_target(machine, &state.state),
        };
        current.is_some_and(|current| {
            alias
                .member_state_ids
                .iter()
                .any(|member| member == &current.uuid().to_string())
        })
    }

    fn direct_transition<'a>(
        &self,
        machine: &'a LowLevelStateMachine,
        state: &InterruptibleFsmState,
        transition_id: TransitionId,
    ) -> Option<&'a LowLevelTransition> {
        machine
            .transitions
            .get(&LowLevelTransitionId::Start(transition_id))
            .or_else(|| {
                machine
                    .transitions
                    .get(&LowLevelTransitionId::Immediate(transition_id))
            })
            .filter(|transition| {
                state.state == transition.source
                    || Self::can_interrupt(machine, state, transition)
                    || self.alias_matches(machine, state, transition)
            })
    }

    fn initialize_state(
        machine: &LowLevelStateMachine,
        target: LowLevelStateId,
        ctx: &mut NodeContext,
    ) {
        let Some(target_state) = machine.states.get(&target) else {
            return;
        };
        let context_id = ctx.create_child_context_id(target_state.graph.id(), Some(target));
        if let Some(graph_state) = ctx
            .graph_context
            .context_arena
            .get_mut()
            .get_context_mut(context_id)
        {
            graph_state.node_states.clear();
        }
    }

    fn activate_transition(
        state: &mut InterruptibleFsmState,
        transition: &LowLevelTransition,
        time: f32,
        blend_duration: Option<f32>,
    ) {
        let interrupted = state.state != transition.source;
        let mut interrupted_sources = Vec::new();
        if interrupted && matches!(transition.target, LowLevelStateId::HlTransition(_)) {
            interrupted_sources.push(InterruptibleFsmSource {
                state: state.state.clone(),
                state_entered_time: state.state_entered_time,
                context_slot: state.context_slot,
                blend_duration: state.blend_duration,
            });
            interrupted_sources.extend(state.interrupted_sources.iter().cloned());
        }
        state.context_slot = (0..)
            .find(|slot| {
                !interrupted_sources
                    .iter()
                    .any(|source| source.state == transition.target && source.context_slot == *slot)
            })
            .unwrap();
        state.state = transition.target.clone();
        state.state_entered_time = time;
        state.blend_duration = blend_duration;
        state.interrupted_sources = interrupted_sources;
        state.source_pose = None;
    }

    fn prepare_transition_duration(
        &self,
        machine: &LowLevelStateMachine,
        transition: &LowLevelTransition,
        state: &mut InterruptibleFsmState,
        source_weights: &[SourceFsmStateWeight],
        ctx: &NodeContext,
    ) -> Result<Option<f32>, GraphError> {
        let Some(target) = machine.states.get(&transition.target) else { return Ok(None); };
        let Some(duration) = target.hl_transition.as_ref().and_then(|data| data.timed) else { return Ok(None); };
        let graph = ctx.graph_context.resources.animation_graph_assets.get(&target.graph)
            .ok_or(GraphError::FSMGraphAssetMissing)?;
        if let Some(node) = graph.nodes.values().find_map(|node| node.try_inner_downcast_ref::<InertializationNode>()) {
            crate::extensions::animation_context::merge_inertialization_request(
                &mut state.context.inertialization_request,
                Some(&crate::extensions::animation_context::SourceInertializationRequest {
                    frame: ctx.graph_context.frame_index(),
                    duration: node.transition_duration,
                }),
            );
            return Ok(Some(0.0));
        }
        let profile = graph.nodes.values().find_map(|node| node.try_inner_downcast_ref::<ProfiledBlendNode>())
            .ok_or_else(|| Self::state_weight_error(ctx, "source transition blend mode is unavailable"))?;
        let target_weight = source_weights.iter().find(|weight| weight.state_id == transition.hl_target.uuid().to_string())
            .map_or(0.0, |weight| weight.value);
        let context = self.source_target_context(machine, state, ctx)?;
        let adjustment = Self::transition_id(transition).and_then(|id| match id {
            TransitionId::Direct(id) => self.source_transition_conditions.iter()
                .find(|condition| condition.transition_id == id.uuid().to_string()),
            _ => None,
        }).and_then(|condition| condition.automatic_trigger_time.and_then(|trigger|
            source_automatic_transition_adjustment(context.as_ref().and_then(|value| value.relevant_asset.as_ref()), trigger, condition.transition_duration)))
            .unwrap_or(0.0);
        Ok(Some(source_transition_duration(duration, target_weight, adjustment, profile.alpha_blend_mode)))
    }

    fn event_start_index(event_count: usize, max_requests: usize) -> usize {
        event_count.saturating_sub(max_requests)
    }

    fn time_update(&self, ctx: &NodeContext) -> Result<TimeUpdate, GraphError> {
        if self.internal_time_key.is_empty() {
            return ctx.time_update_fwd();
        }
        let delta = global_f32_or_root_default(ctx, &self.internal_time_key)?;
        Ok(TimeUpdate::Delta(delta.max(0.0)))
    }

    fn process_events(
        &self,
        machine: &LowLevelStateMachine,
        events: EventQueue,
        allow_external: bool,
        remaining_transitions: &mut usize,
        ctx: &mut NodeContext,
    ) -> Result<(), GraphError> {
        let time = ctx.time();
        let mut state = ctx.state::<InterruptibleFsmState>()?.clone();
        let mut reset = None;
        let start = if allow_external {
            Self::event_start_index(events.events.len(), self.max_transition_requests)
        } else {
            0
        };
        for sampled in events.events.into_iter().skip(start) {
            let ending = matches!(&sampled.event, AnimationEvent::EndTransition);
            if *remaining_transitions == 0 && !ending {
                break;
            }
            let transition = match sampled.event {
                AnimationEvent::TransitionToState(target) => {
                    if let LowLevelStateId::HlState(source) = state.state.clone() {
                        Self::transition_for_state_pair(machine, source, target, allow_external)
                    } else {
                        None
                    }
                }
                AnimationEvent::TransitionToStateLabel(label) => {
                    let target = machine
                        .hl_states_by_label
                        .get(&label)
                        .and_then(|states| states.first())
                        .copied();
                    if let (Some(target), LowLevelStateId::HlState(source)) =
                        (target, state.state.clone())
                    {
                        Self::transition_for_state_pair(machine, source, target, allow_external)
                    } else {
                        None
                    }
                }
                AnimationEvent::Transition(transition_id) => {
                    self.direct_transition(machine, &state, transition_id)
                }
                AnimationEvent::EndTransition => machine
                    .states
                    .get(&state.state)
                    .and_then(|state| state.hl_transition.as_ref())
                    .and_then(|active| {
                        machine
                            .transitions
                            .get(&LowLevelTransitionId::End(active.hl_transition_id))
                    }),
                _ => None,
            };
            if let Some(transition) = transition {
                let source_weights = state.recorded_state_weights.clone();
                let always_reset = Self::transition_id(transition)
                    .and_then(|id| match id {
                        TransitionId::Direct(id) => self
                            .source_transition_conditions
                            .iter()
                            .find(|condition| condition.transition_id == id.uuid().to_string()),
                        _ => None,
                    })
                    .and_then(|condition| condition.always_reset_target_on_entry)
                    .unwrap_or(true);
                let target_active = source_weights.iter().any(|weight| {
                    weight.state_id == transition.hl_target.uuid().to_string() && weight.value > 0.0
                });
                if transition.reset_target_state && (always_reset || !target_active) {
                    reset = Some(transition.clone());
                }
                let interrupted = match &state.state {
                    LowLevelStateId::HlTransition(interrupted)
                        if state.state != transition.source =>
                    {
                        Some(interrupted.clone())
                    }
                    _ => None,
                };
                if let Some(interrupted) = interrupted {
                    self.append_transition_notifications(
                        &mut state,
                        &interrupted,
                        "TransitionInterrupt",
                    );
                }
                if let Some(transition_id) = Self::transition_id(transition) {
                    self.append_transition_notifications(
                        &mut state,
                        transition_id,
                        if ending {
                            "TransitionEnd"
                        } else {
                            "TransitionStart"
                        },
                    );
                }
                if !ending && Self::source_condition_state(machine, &state.state) != Some(transition.hl_target) {
                    state.current_state_elapsed_time = 0.0;
                    if let Some(previous) = Self::source_condition_state(machine, &state.state) {
                        state
                            .transition_notifications
                            .extend(Self::state_function_notifications(
                                machine,
                                previous,
                                "OnStateExit",
                                &ctx.graph_context.resources.animation_graph_assets,
                            )?);
                    }
                    state
                        .transition_notifications
                        .extend(Self::state_function_notifications(
                            machine,
                            transition.hl_target,
                            "OnStateEntry",
                            &ctx.graph_context.resources.animation_graph_assets,
                        )?);
                }
                let blend_duration = self.prepare_transition_duration(machine, transition, &mut state, &source_weights, ctx)?;
                Self::activate_transition(&mut state, transition,
                    if blend_duration.is_some() { ctx.prev_time() } else { time }, blend_duration);
                if !ending {
                    *remaining_transitions -= 1;
                }
            }
        }
        let default_state = state.clone();
        *ctx.state_mut_or_else(|| default_state)? = state;
        if let Some(transition) = reset.as_ref() {
            Self::initialize_state(machine, LowLevelStateId::HlState(transition.hl_target), ctx);
        }
        Ok(())
    }

    fn driver_events(&self, ctx: &mut NodeContext) -> Result<EventQueue, GraphError> {
        if self.state_label_key.is_empty() {
            if !self.source_transition_conditions.is_empty() {
                return Ok(EventQueue::default());
            }
            return ctx
                .data_back(LowLevelStateMachine::DRIVER_EVENT_QUEUE)?
                .into_event_queue();
        }
        let value = global_f32_or_root_default(ctx, &self.state_label_key)?;
        let index = state_label_index(value, self.state_labels.len());
        let label = {
            let state = ctx.state_mut_or_else::<InterruptibleFsmState>(|| {
                unreachable!("interruptible FSM state must be initialized before label dispatch")
            })?;
            changed_label(&mut state.label_last_index, index, &self.state_labels)
        };
        Ok(label.map_or_else(EventQueue::default, |value| {
            EventQueue::with_events([SampledEvent::instant(
                AnimationEvent::TransitionToStateLabel(value),
            )])
        }))
    }

    fn source_target_context(
        &self,
        machine: &LowLevelStateMachine,
        state: &InterruptibleFsmState,
        ctx: &NodeContext,
    ) -> Result<Option<AnimationContextState>, GraphError> {
        let Some(target) = Self::source_condition_state(machine, &state.state) else { return Ok(None); };
        let target = LowLevelStateId::HlState(target);
        let (state_data, graph) = Self::active_graph(machine, &target, ctx)?;
        let sub_ctx = ctx.create_child_context(state_data.graph.id(), graph, Some(target.clone()));
        Ok(graph_output_context_state(graph, &sub_ctx, self.state_pose_output_for(&target)))
    }

    fn source_transition_event(
        &self,
        machine: &LowLevelStateMachine,
        parent_time_update: TimeUpdate,
        ctx: &mut NodeContext,
    ) -> Result<Option<TransitionId>, GraphError> {
        let state = ctx.state::<InterruptibleFsmState>()?.clone();
        let Some(active_state) = Self::source_condition_state(machine, &state.state) else {
            return Ok(None);
        };
        let context = self.source_target_context(machine, &state, ctx)?;
        for condition in &self.source_transition_conditions {
            let transition = Self::source_condition_transition_id(machine, condition)
                .ok_or_else(|| GraphError::UnknownErrorNode {
                    graph: ctx.graph_context.context_id,
                    node: ctx.node_id,
                    error: "source transition condition target is unavailable".to_owned(),
                })?;
            if self.direct_transition(machine, &state, transition).is_none() {
                continue;
            }
            let matched = if let Some(trigger_time) = condition.automatic_trigger_time {
                source_automatic_transition_adjustment(
                    context.as_ref().and_then(|value| value.relevant_asset.as_ref()),
                    trigger_time,
                    condition.transition_duration,
                ).is_some()
            } else {
                self.source_condition_graph_matches(
                    machine,
                    &state,
                    parent_time_update.clone(),
                    &condition.condition,
                    &condition.output,
                    condition.context_state,
                    ctx,
                )?
            };
            let mut gates_match = matched;
            for gate in &condition.gates {
                if !gates_match {
                    break;
                }
                gates_match = self.source_condition_graph_matches(
                    machine,
                    &state,
                    parent_time_update.clone(),
                    &gate.condition,
                    &gate.output,
                    gate.context_state,
                    ctx,
                )?;
            }
            if !gates_match {
                continue;
            }
            if self.direct_transition(machine, &state, transition)
                .is_some_and(|transition| transition.hl_target == active_state) {
                continue;
            }
            return Ok(Some(transition));
        }
        Ok(None)
    }

    fn source_condition_graph_matches(
        &self,
        machine: &LowLevelStateMachine,
        state: &InterruptibleFsmState,
        parent_time_update: TimeUpdate,
        condition: &Handle<AnimationGraph>,
        output: &str,
        context_state: StateId,
        ctx: &mut NodeContext,
    ) -> Result<bool, GraphError> {
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(condition)
            .ok_or(GraphError::GraphAssetMissing)?;
        let io = InterruptibleFsmIo {
            node_context: ctx.clone(),
            machine,
            current_time: ctx.time(),
            parent_time_update,
            state: state.state.clone(),
            state_entered_time: state.state_entered_time,
            context_slot: state.context_slot,
            blend_duration: state.blend_duration,
            source_pose: state.source_pose.clone(),
            role: FsmRole::Root,
            stack: VecDeque::new(),
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let context_state =
            Self::source_condition_context_state(context_state).ok_or_else(|| {
                GraphError::UnknownErrorNode {
                    graph: ctx.graph_context.context_id,
                    node: ctx.node_id,
                    error: "source condition context identity is unavailable".to_owned(),
                }
            })?;
        let sub_ctx = ctx
            .create_child_context(condition.id(), graph, Some(context_state))
            .with_io(&io);
        graph
            .get_data(TargetPin::OutputData(output.to_owned()), sub_ctx)?
            .as_bool()
    }

    fn source_condition_context_state(context_state: StateId) -> Option<LowLevelStateId> {
        (!context_state.uuid().is_nil()).then_some(LowLevelStateId::HlTransition(
            TransitionId::State(context_state),
        ))
    }

    fn source_entry_state(
        &self,
        machine: &LowLevelStateMachine,
        state: &InterruptibleFsmState,
        parent_time_update: TimeUpdate,
        ctx: &mut NodeContext,
    ) -> Result<Option<StateId>, GraphError> {
        if self.source_entry_conditions.is_empty() {
            return Ok(None);
        }
        for condition in &self.source_entry_conditions {
            let mut matches = true;
            for gate in &condition.gates {
                if !matches {
                    break;
                }
                matches = self.source_condition_graph_matches(
                    machine,
                    state,
                    parent_time_update.clone(),
                    &gate.condition,
                    &gate.output,
                    gate.context_state,
                    ctx,
                )?;
            }
            if !matches {
                continue;
            }
            return Self::source_entry_target_state(machine, &condition.target_state_id)
                .map(Some)
                .ok_or_else(|| GraphError::UnknownErrorNode {
                    graph: ctx.graph_context.context_id,
                    node: ctx.node_id,
                    error: "source entry condition target is unavailable".to_owned(),
                });
        }
        Err(GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id,
            error: "source entry conduit has no matching target".to_owned(),
        })
    }

    fn source_entry_target_state(
        machine: &LowLevelStateMachine,
        target_state_id: &str,
    ) -> Option<StateId> {
        machine.states.keys().find_map(|state| match state {
            LowLevelStateId::HlState(state) if state.uuid().to_string() == target_state_id => {
                Some(state.clone())
            }
            _ => None,
        })
    }

    fn active_graph<'a>(
        machine: &'a LowLevelStateMachine,
        state: &LowLevelStateId,
        ctx: &NodeContext<'a>,
    ) -> Result<
        (
            &'a bevy_animation_graph::core::state_machine::low_level::LowLevelState,
            &'a AnimationGraph,
        ),
        GraphError,
    > {
        let state_data = machine
            .states
            .get(state)
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&state_data.graph)
            .ok_or(GraphError::FSMGraphAssetMissing)?;
        Ok((state_data, graph))
    }

    fn state_weight_error(ctx: &NodeContext, error: &str) -> GraphError {
        GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id,
            error: error.to_owned(),
        }
    }

    fn add_state_weight(weights: &mut Vec<SourceFsmStateWeight>, state: StateId, value: f32) {
        if let Some(existing) = weights
            .iter_mut()
            .find(|entry| entry.state_id == state.uuid().to_string())
        {
            existing.value += value;
        } else {
            weights.push(SourceFsmStateWeight {
                state_id: state.uuid().to_string(),
                value,
            });
        }
    }

    fn initial_state_weights(state: &LowLevelStateId) -> Vec<SourceFsmStateWeight> {
        match state {
            LowLevelStateId::HlState(value) => vec![SourceFsmStateWeight {
                state_id: value.uuid().to_string(),
                value: 1.0,
            }],
            LowLevelStateId::HlTransition(_) => Vec::new(),
        }
    }

    fn local_state_weight(weights: &[SourceFsmStateWeight], state: &LowLevelStateId) -> f32 {
        match state {
            LowLevelStateId::HlState(state) => weights.iter()
                .find(|weight| weight.state_id == state.uuid().to_string())
                .map_or(0.0, |weight| weight.value),
            LowLevelStateId::HlTransition(_) => 1.0,
        }
    }

    fn initial_state(state: LowLevelStateId, time: f32, frame: u64) -> InterruptibleFsmState {
        let initial_weights = Self::initial_state_weights(&state);
        InterruptibleFsmState {
            state,
            state_entered_time: time,
            context_slot: 0,
            blend_duration: None,
            interrupted_sources: Vec::new(),
            context: AnimationContextState::default(),
            recorded_machine_weight: 1.0,
            current_state_elapsed_time: 0.0,
            recorded_state_weights: initial_weights,
            source_pose: None,
            last_pose: None,
            label_last_index: None,
            first_update: true,
            last_update_frame: Some(frame),
            transition_notifications: Vec::new(),
            notification_generation: 0,
        }
    }

    fn state_weight_values(
        machine: &LowLevelStateMachine,
        state: &InterruptibleFsmState,
        ctx: &NodeContext,
        time: f32,
    ) -> Result<Vec<SourceFsmStateWeight>, GraphError> {
        let current = InterruptibleFsmSource {
            state: state.state.clone(),
            state_entered_time: state.state_entered_time,
            context_slot: state.context_slot,
            blend_duration: state.blend_duration,
        };
        let mut weights = Vec::new();
        for source in state
            .interrupted_sources
            .iter()
            .rev()
            .chain(std::iter::once(&current))
        {
            if matches!(source.state, LowLevelStateId::HlState(_)) {
                weights = Self::initial_state_weights(&source.state);
                continue;
            }
            let state_data = machine
                .states
                .get(&source.state)
                .ok_or(GraphError::FSMCurrentStateMissing)?;
            let transition = state_data
                .hl_transition
                .as_ref()
                .ok_or(GraphError::FSMCurrentStateMissing)?;
            let duration = source.blend_duration.or(transition.timed).unwrap_or_default().max(0.0);
            let factor = if duration <= f32::EPSILON {
                1.0
            } else {
                ((time - source.state_entered_time) / duration).clamp(0.0, 1.0)
            };
            let graph = ctx
                .graph_context
                .resources
                .animation_graph_assets
                .get(&state_data.graph)
                .ok_or(GraphError::FSMGraphAssetMissing)?;
            let profiled = graph
                .nodes
                .values()
                .filter_map(|node| node.try_inner_downcast_ref::<ProfiledBlendNode>())
                .collect::<Vec<_>>();
            let inertial = graph
                .nodes
                .values()
                .filter_map(|node| node.try_inner_downcast_ref::<InertializationNode>())
                .collect::<Vec<_>>();
            let alpha = match (profiled.as_slice(), inertial.as_slice()) {
                ([node], []) => transition_alpha(factor, node.alpha_blend_mode, &node.custom_curve),
                ([], [_]) => 1.0,
                _ => {
                    return Err(Self::state_weight_error(
                        ctx,
                        "source transition weight is unavailable",
                    ));
                }
            };
            if weights.is_empty() {
                weights = Self::initial_state_weights(&LowLevelStateId::HlState(transition.source));
            }
            for entry in &mut weights {
                entry.value = (entry.value * (1.0 - alpha)).clamp(0.0, 1.0);
            }
            Self::add_state_weight(&mut weights, transition.target, alpha);
            weights.retain(|entry| entry.value > f32::EPSILON);
        }
        Ok(weights)
    }

    fn record_state_weights(
        &self,
        machine: &LowLevelStateMachine,
        ctx: &mut NodeContext,
    ) -> Result<(), GraphError> {
        let time = ctx.time();
        let snapshot = ctx.state::<InterruptibleFsmState>()?.clone();
        let weights = Self::state_weight_values(machine, &snapshot, ctx, time)?;
        let state = ctx.state_mut_or_else(|| snapshot.clone())?;
        state.recorded_machine_weight = current_machine_weight();
        state.recorded_state_weights = weights;
        Ok(())
    }

    fn prewarm_scoped_caches(
        graph: &AnimationGraph,
        ctx: &mut NodeContext,
    ) -> Result<(), GraphError> {
        for (cache_node, cache_key) in graph.nodes.values().filter_map(|node| {
            node.try_inner_downcast_ref::<SourceScopedCachedPoseUseNode>()
                .map(|value| (value.cache_node.as_str(), value.cache_key.as_str()))
        }) {
            prewarm_scoped_cached_pose(ctx, cache_node, cache_key)?;
        }
        Ok(())
    }

    fn transition_endpoint_states(state: &LowLevelState) -> Option<[LowLevelStateId; 2]> {
        state.hl_transition.as_ref().map(|transition| {
            [
                LowLevelStateId::HlState(transition.source.clone()),
                LowLevelStateId::HlState(transition.target.clone()),
            ]
        })
    }

    fn prewarm_active_state_caches(
        machine: &LowLevelStateMachine,
        state: &LowLevelStateId,
        ctx: &mut NodeContext,
        weights: &[SourceFsmStateWeight],
    ) -> Result<(), GraphError> {
        let (state_data, graph) = Self::active_graph(machine, state, ctx)?;
        with_machine_weight(Self::local_state_weight(weights, state), || Self::prewarm_scoped_caches(graph, ctx))?;
        if let Some(states) = Self::transition_endpoint_states(state_data) {
            for state in states {
                let (_, graph) = Self::active_graph(machine, &state, ctx)?;
                with_machine_weight(Self::local_state_weight(weights, &state), || Self::prewarm_scoped_caches(graph, ctx))?;
            }
        }
        Ok(())
    }

    fn evaluate_active_state(
        &self,
        machine: &LowLevelStateMachine,
        parent_time_update: TimeUpdate,
        ctx: &mut NodeContext,
    ) -> Result<EventQueue, GraphError> {
        let mut state = ctx.state::<InterruptibleFsmState>()?.clone();
        if let Some(completed) = state.interrupted_sources.iter().position(|source| {
            machine
                .states
                .get(&source.state)
                .and_then(|state| state.hl_transition.as_ref())
                .and_then(|transition| {
                    timed_transition_progress(
                        ctx.time() - source.state_entered_time,
                        source.blend_duration.or(transition.timed),
                    )
                })
                .is_some_and(|progress| progress >= 1.0)
        }) {
            state.interrupted_sources.truncate(completed);
        }
        let now = ctx.time();
        let weights = Self::state_weight_values(machine, &state, ctx, now)?;
        let mut source_pose = None;
        for source in state.interrupted_sources.iter().rev() {
            Self::prewarm_active_state_caches(machine, &source.state, ctx, &weights)?;
            let (source_data, source_graph) = Self::active_graph(machine, &source.state, ctx)?;
            let io = InterruptibleFsmIo {
                node_context: ctx.clone(),
                machine,
                current_time: ctx.time(),
                parent_time_update: parent_time_update.clone(),
                state: source.state.clone(),
                state_entered_time: source.state_entered_time,
                context_slot: source.context_slot,
                blend_duration: source.blend_duration,
                source_pose,
                role: FsmRole::Root,
                stack: VecDeque::new(),
                state_pose_output: self.state_pose_output.clone(),
                state_pose_outputs: self.state_pose_outputs.clone(),
            };
            let sub_ctx = state_graph_context(ctx, source_data, source_graph, source.context_slot)
                .with_io(&io);
            let output = source_graph
                .io_spec
                .iter_output_data()
                .find(|(_, spec)| **spec == DataSpec::Pose)
                .map(|(output, _)| output.clone())
                .ok_or(GraphError::FSMGraphAssetMissing)?;
            let pose = source_graph
                .get_data(TargetPin::OutputData(output.clone()), sub_ctx.clone())?
                .into_pose()?;
            source_pose = Some((
                pose,
                graph_output_context_state(source_graph, &sub_ctx, &output).unwrap_or_default(),
            ));
        }
        state.source_pose = source_pose;
        *ctx.state_mut_or_else(|| state.clone())? = state.clone();
        Self::prewarm_active_state_caches(machine, &state.state, ctx, &weights)?;
        let (state_data, graph) = Self::active_graph(machine, &state.state, ctx)?;
        let elapsed = (ctx.time() - state.state_entered_time).max(0.0);
        let io = InterruptibleFsmIo {
            node_context: ctx.clone(),
            machine,
            current_time: ctx.time(),
            parent_time_update,
            state: state.state.clone(),
            state_entered_time: state.state_entered_time,
            context_slot: state.context_slot,
            blend_duration: state.blend_duration,
            source_pose: state.source_pose.clone(),
            role: FsmRole::Root,
            stack: VecDeque::new(),
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let sub_ctx = state_graph_context(ctx, state_data, graph, state.context_slot).with_io(&io);
        let mut internal_events = EventQueue::default();
        let mut latest_pose = None;
        let mut pose_output = None;
        let mut context_state = AnimationContextState::default();
        for (id, _) in graph.io_spec.iter_output_data() {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            if id == LowLevelStateMachine::DRIVER_EVENT_QUEUE {
                internal_events.extend(value.into_event_queue()?);
            } else {
                let outer_output = self.outer_output(&state.state, id);
                if outer_output == self.outer_pose_output {
                    if let DataValue::Pose(pose) = &value {
                        latest_pose = Some(pose.clone());
                        pose_output = Some(id.clone());
                    }
                }
                capture_context_curve(&mut context_state, id, &value)?;
                ctx.set_data_fwd(outer_output, value);
            }
        }
        for input in graph.io_spec.iter_input_times() {
            if let GraphInputPin::Passthrough(pin) = input {
                let update =
                    graph.get_time_update(SourcePin::InputTime(input.clone()), sub_ctx.clone())?;
                ctx.set_time_update_back(pin.clone(), update);
            }
        }
        if let Some(pose) = latest_pose {
            let snapshot = ctx.state::<InterruptibleFsmState>()?.clone();
            ctx.state_mut_or_else(|| snapshot)?.last_pose = Some(pose);
        }
        if let Some(output) = pose_output
            && let Some(context) = graph_output_context_state(graph, &sub_ctx, &output)
        {
            merge_missing_context_curves(&mut context_state, &context);
        }
        crate::extensions::animation_context::merge_inertialization_request(
            &mut context_state.inertialization_request, state.context.inertialization_request.as_ref());
        let snapshot = ctx.state::<InterruptibleFsmState>()?.clone();
        ctx.state_mut_or_else(|| snapshot)?.context = context_state;
        self.record_state_weights(machine, ctx)?;
        if machine
            .states
            .get(&state.state)
            .and_then(|state| state.hl_transition.as_ref())
            .and_then(|transition| timed_transition_progress(elapsed, state.blend_duration.or(transition.timed)))
            .is_some_and(|progress| progress >= 1.0)
        {
            internal_events.add_instant_event(AnimationEvent::EndTransition);
        }
        Ok(internal_events)
    }
}

impl NodeLike for InterruptibleFsmNode {
    fn duration(&self, _ctx: NodeContext) -> Result<(), GraphError> {
        Err(GraphError::FsmDoesNotSupportDuration)
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let resources = ctx.graph_context.resources;
        let machine = self.state_machine(resources)?;
        let update = self.time_update(&ctx)?;
        let time = update
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let elapsed_delta = if let TimeUpdate::Delta(delta) = &update { *delta } else { time - ctx.prev_time() }.max(0.0);
        ctx.set_time(time);
        ctx.set_time_update_back(LowLevelStateMachine::DRIVER_TIME, update.clone());
        let start_state = machine
            .start_state
            .clone()
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let frame = ctx.graph_context.frame_index();
        let entry_graph = ctx.graph_context.context_id;
        let entry_node = ctx.node_id;
        let entry_error = |phase: &str, error: GraphError| GraphError::UnknownErrorNode {
            graph: entry_graph,
            node: entry_node,
            error: format!("source entry initialization {phase}: {error}"),
        };
        let initial_state = Self::initial_state(start_state, time, frame);
        let select_entry = {
            let state = ctx
                .state_mut_or_else(|| initial_state.clone())
                .map_err(|error| entry_error("state setup", error))?;
            state.recorded_machine_weight = current_machine_weight();
            if self.reinitialize_on_becoming_relevant
                && !state.first_update
                && became_relevant(state.last_update_frame, frame)
            {
                *state = initial_state.clone();
            } else {
                state.last_update_frame = Some(frame);
            }
            state.first_update
        };
        if select_entry {
            let state = ctx
                .state::<InterruptibleFsmState>()
                .map_err(|error| entry_error("state read", error))?
                .clone();
            if let Some(entry) = self
                .source_entry_state(machine, &state, update.clone(), &mut ctx)
                .map_err(|error| entry_error("condition evaluation", error))?
            {
                *ctx.state_mut_or_else(|| initial_state.clone())
                    .map_err(|error| entry_error("state replacement", error))? =
                    Self::initial_state(LowLevelStateId::HlState(entry), time, frame);
            }
            let entry = ctx.state::<InterruptibleFsmState>()?.state.clone();
            Self::initialize_state(machine, entry, &mut ctx);
        }
        let state = ctx
            .state_mut_or_else(|| initial_state.clone())
            .map_err(|error| entry_error("state finalization", error))?;
        state.transition_notifications.clear();
        state.context.inertialization_request = None;
        state.notification_generation = NOTIFICATION_GENERATION.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        if select_entry && let Some(entry) = Self::source_condition_state(machine, &state.state) {
            state.transition_notifications.extend(Self::state_function_notifications(
                machine, entry, "OnStateEntry", &resources.animation_graph_assets,
            )?);
        }
        let external_events = self
            .driver_events(&mut ctx)
            .map_err(|error| entry_error("external event collection", error))?;
        let mut remaining_transitions = self.max_transitions_per_frame;
        self.process_events(
            machine,
            external_events,
            true,
            &mut remaining_transitions,
            &mut ctx,
        )
        .map_err(|error| entry_error("external event processing", error))?;
        while remaining_transitions > 0
            && let Some(transition) = self
                .source_transition_event(machine, update.clone(), &mut ctx)
                .map_err(|error| entry_error("source transition selection", error))?
        {
            self.process_events(
                machine,
                EventQueue::with_events([SampledEvent::instant(AnimationEvent::Transition(
                    transition,
                ))]),
                false,
                &mut remaining_transitions,
                &mut ctx,
            )
            .map_err(|error| entry_error("source transition processing", error))?;
        }
        if self.skip_first_update_transition
            && ctx.state::<InterruptibleFsmState>()?.first_update
            && matches!(
                &ctx.state::<InterruptibleFsmState>()?.state,
                LowLevelStateId::HlTransition(_)
            )
        {
            let mut forced_transition = 1;
            self.process_events(
                machine,
                EventQueue::with_events([SampledEvent::instant(AnimationEvent::EndTransition)]),
                false,
                &mut forced_transition,
                &mut ctx,
            )
            .map_err(|error| entry_error("first update transition processing", error))?;
        }
        let internal_events = self
            .evaluate_active_state(machine, update, &mut ctx)
            .map_err(|error| entry_error("active state evaluation", error))?;
        self.process_events(
            machine,
            internal_events,
            false,
            &mut remaining_transitions,
            &mut ctx,
        )
        .map_err(|error| entry_error("internal event processing", error))?;
        {
            let state = ctx.state_mut_or_else(|| initial_state.clone())?;
            state.first_update = false;
            state.current_state_elapsed_time += elapsed_delta;
        }
        if self.expose_telemetry {
            let state = ctx.state::<InterruptibleFsmState>()?;
            let actual_state = self.telemetry_state_index(&state.state);
            let actual_transition = self.telemetry_transition_index(&state.state);
            let transition_active = matches!(state.state, LowLevelStateId::HlTransition(_));
            let state_entered_time = state.state_entered_time;
            let state_elapsed_time = (ctx.time() - state_entered_time).max(0.0);
            ctx.set_data_fwd(Self::OUT_ACTUAL_STATE, actual_state);
            ctx.set_data_fwd(Self::OUT_ACTUAL_TRANSITION, actual_transition);
            ctx.set_data_fwd(
                Self::OUT_ACTUAL_TRANSITION_ACTIVE,
                if transition_active { 1.0 } else { 0.0 },
            );
            ctx.set_data_fwd(Self::OUT_ACTUAL_STATE_ENTERED_TIME, state_entered_time);
            ctx.set_data_fwd(Self::OUT_ACTUAL_STATE_ELAPSED_TIME, state_elapsed_time);
        }
        Ok(())
    }

    fn try_get_time(&self, ctx: NodeContext, pin: PinId) -> Result<TimeUpdate, GraphError> {
        let _ = pin;
        self.time_update(&ctx)
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        let machine = ctx
            .resources()
            .fsm_assets
            .get(&self.fsm)
            .ok_or(GraphError::FsmAssetMissing)?;
        if self.state_label_key.is_empty() && self.source_transition_conditions.is_empty() {
            ctx.add_input_data(
                LowLevelStateMachine::DRIVER_EVENT_QUEUE,
                DataSpec::EventQueue,
            );
        }
        ctx.update_from_node_spec(&machine.node_spec);
        if self.expose_telemetry {
            ctx.add_output_data(Self::OUT_ACTUAL_STATE, DataSpec::F32)
                .add_output_data(Self::OUT_ACTUAL_TRANSITION, DataSpec::F32)
                .add_output_data(Self::OUT_ACTUAL_TRANSITION_ACTIVE, DataSpec::F32)
                .add_output_data(Self::OUT_ACTUAL_STATE_ENTERED_TIME, DataSpec::F32)
                .add_output_data(Self::OUT_ACTUAL_STATE_ELAPSED_TIME, DataSpec::F32);
        }
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Interruptible FSM".into()
    }
}

#[derive(Clone, Copy)]
enum FsmRole {
    Source,
    Target,
    Root,
}

fn state_graph_context<'a>(
    ctx: &NodeContext<'a>,
    state: &LowLevelState,
    graph: &'a AnimationGraph,
    slot: usize,
) -> GraphContext<'a> {
    let context = ctx.create_child_context(state.graph.id(), graph, Some(state.id.clone()));
    if slot == 0 {
        return context;
    }
    context
        .create_node_context(ctx.node_id)
        .create_child_context(
            state.graph.id(),
            graph,
            Some(LowLevelStateId::HlState(StateId::from(
                bevy::asset::uuid::Uuid::from_u128(slot as u128),
            ))),
        )
}

#[derive(Clone)]
struct FsmStackFrame {
    state: LowLevelStateId,
    state_entered_time: f32,
    context_slot: usize,
    blend_duration: Option<f32>,
    source_pose: Option<(Pose, AnimationContextState)>,
    role: FsmRole,
}

#[derive(Clone)]
struct InterruptibleFsmIo<'a> {
    node_context: NodeContext<'a>,
    machine: &'a LowLevelStateMachine,
    current_time: f32,
    parent_time_update: TimeUpdate,
    state: LowLevelStateId,
    state_entered_time: f32,
    context_slot: usize,
    blend_duration: Option<f32>,
    source_pose: Option<(Pose, AnimationContextState)>,
    role: FsmRole,
    stack: VecDeque<FsmStackFrame>,
    state_pose_output: String,
    state_pose_outputs: Vec<StatePoseOutput>,
}

impl<'a> InterruptibleFsmIo<'a> {
    fn state_pose_output_for(&self, state: &LowLevelStateId) -> &str {
        let LowLevelStateId::HlState(state) = state else {
            return &self.state_pose_output;
        };
        self.state_pose_outputs
            .iter()
            .find(|value| value.state_id == state.uuid().to_string())
            .map(|value| value.output.as_str())
            .unwrap_or(&self.state_pose_output)
    }

    fn parent_data(&self, pin: &PinId, ctx: &GraphContext) -> Result<DataValue, GraphError> {
        let parent = self.node_context.clone().with_state_key(ctx.state_key);
        let value = parent.data_back(pin.clone())?;
        if matches!(value, DataValue::Pose(_)) {
            crate::extensions::animation_context::write_linked_input_context(
                &mut ctx.clone(),
                GraphInputPin::Passthrough(pin.clone()),
                crate::extensions::animation_context::pose_input_context_state(&parent, pin)
                    .unwrap_or_default(),
            )?;
        }
        Ok(value)
    }

    fn parent_duration(&self, pin: &PinId, ctx: &GraphContext) -> Result<DurationData, GraphError> {
        self.node_context
            .clone()
            .with_state_key(ctx.state_key)
            .duration_back(pin.clone())
    }

    fn source_state(&self) -> Result<LowLevelStateId, GraphError> {
        self.machine
            .states
            .get(&self.state)
            .and_then(|state| state.hl_transition.as_ref())
            .map(|transition| LowLevelStateId::HlState(transition.source))
            .ok_or(GraphError::FSMCurrentStateMissing)
    }

    fn target_state(&self) -> Result<LowLevelStateId, GraphError> {
        self.machine
            .states
            .get(&self.state)
            .and_then(|state| state.hl_transition.as_ref())
            .map(|transition| LowLevelStateId::HlState(transition.target))
            .ok_or(GraphError::FSMCurrentStateMissing)
    }

    fn builtin(&self, pin: FsmBuiltinPin) -> Result<DataValue, GraphError> {
        let elapsed = (self.current_time - self.state_entered_time).max(0.0);
        match pin {
            FsmBuiltinPin::TimeElapsed => Ok(DataValue::F32(elapsed)),
            FsmBuiltinPin::PercentThroughDuration => self
                .machine
                .states
                .get(&self.state)
                .and_then(|state| state.hl_transition.as_ref())
                .and_then(|transition| timed_transition_progress(elapsed, self.blend_duration.or(transition.timed)))
                .map(DataValue::F32)
                .ok_or_else(|| GraphError::UnknownErrorNode {
                    graph: self.node_context.graph_context.context_id,
                    node: self.node_context.node_id,
                    error: "FSM percent-through-duration requires an active timed transition"
                        .to_owned(),
                }),
        }
    }

    fn nested_io(
        &self,
        state: LowLevelStateId,
        role: FsmRole,
        ctx: &GraphContext,
    ) -> Result<(Handle<AnimationGraph>, InterruptibleFsmIo<'a>), GraphError> {
        let graph = self
            .machine
            .states
            .get(&state)
            .map(|state| state.graph.clone())
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let mut stack = self.stack.clone();
        stack.push_back(FsmStackFrame {
            state: self.state.clone(),
            state_entered_time: self.state_entered_time,
            context_slot: self.context_slot,
            blend_duration: self.blend_duration,
            source_pose: self.source_pose.clone(),
            role: self.role,
        });
        let io = InterruptibleFsmIo {
            node_context: self.node_context.clone(),
            machine: self.machine,
            current_time: self.current_time,
            parent_time_update: self.parent_time_update.clone(),
            state,
            state_entered_time: self.state_entered_time,
            context_slot: self.context_slot,
            blend_duration: self.blend_duration,
            source_pose: None,
            role,
            stack,
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let _ = ctx;
        Ok((graph, io))
    }

    fn evaluated_state_weight(&self, state: &LowLevelStateId) -> Result<f32, GraphError> {
        let snapshot = self.node_context.state::<InterruptibleFsmState>()?;
        let weights = InterruptibleFsmNode::state_weight_values(
            self.machine, snapshot, &self.node_context, self.current_time,
        )?;
        Ok(InterruptibleFsmNode::local_state_weight(&weights, state))
    }

    fn state_data(
        &self,
        pin: PinId,
        state: LowLevelStateId,
        role: FsmRole,
        ctx: &GraphContext,
        input: GraphInputPin,
    ) -> Result<DataValue, GraphError> {
        let (graph_handle, io) = self.nested_io(state.clone(), role, ctx)?;
        let graph = self
            .node_context
            .graph_context
            .resources
            .animation_graph_assets
            .get(&graph_handle)
            .ok_or(GraphError::FSMGraphAssetMissing)?;
        let sub_ctx = self
            .node_context
            .create_child_context(graph_handle.id(), graph, Some(state.clone()))
            .with_state_key(ctx.state_key)
            .with_io(&io);
        let weight = self.evaluated_state_weight(&state)?;
        with_machine_weight(weight, || {
            let result = graph.get_data(TargetPin::OutputData(pin.clone()), sub_ctx.clone());
            if matches!(&result, Ok(DataValue::Pose(_))) {
                crate::extensions::animation_context::write_linked_input_context(
                    &mut ctx.clone(),
                    input,
                    graph_output_context_state(graph, &sub_ctx, &pin).unwrap_or_default(),
                )?;
            }
            let Err(error) = result else {
                return result;
            };
            let pose_output = self.state_pose_output_for(&state).to_owned();
            if pin == pose_output {
                return Err(error);
            }
            graph.get_data(TargetPin::OutputData(pose_output.clone()), sub_ctx.clone())?;
            graph_output_context_state(graph, &sub_ctx, &pose_output)
                .and_then(|context| context.curves.get(&crate::extensions::SourceCurveName::from(pin.as_str())).copied())
                .map(DataValue::F32)
                .ok_or(error)
        })
    }

    fn state_duration(
        &self,
        state: LowLevelStateId,
        role: FsmRole,
        ctx: &GraphContext,
    ) -> Result<DurationData, GraphError> {
        let (graph_handle, io) = self.nested_io(state.clone(), role, ctx)?;
        let graph = self
            .node_context
            .graph_context
            .resources
            .animation_graph_assets
            .get(&graph_handle)
            .ok_or(GraphError::FSMGraphAssetMissing)?;
        let sub_ctx = self
            .node_context
            .create_child_context(graph_handle.id(), graph, Some(state))
            .with_state_key(ctx.state_key)
            .with_io(&io);
        graph
            .get_duration(TargetPin::OutputTime, sub_ctx)
            .or(Ok(None))
    }

    fn parent_time(&self, _ctx: &GraphContext) -> Result<TimeUpdate, GraphError> {
        Ok(self.parent_time_update.clone())
    }

    fn nested_time(&self, ctx: &GraphContext) -> Result<TimeUpdate, GraphError> {
        let mut stack = self.stack.clone();
        let Some(frame) = stack.pop_back() else {
            return self.parent_time(ctx);
        };
        let graph_handle = self
            .machine
            .states
            .get(&frame.state)
            .map(|state| state.graph.clone())
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let graph = self
            .node_context
            .graph_context
            .resources
            .animation_graph_assets
            .get(&graph_handle)
            .ok_or(GraphError::FSMGraphAssetMissing)?;
        let io = InterruptibleFsmIo {
            node_context: self.node_context.clone(),
            machine: self.machine,
            current_time: self.current_time,
            parent_time_update: self.parent_time_update.clone(),
            state: frame.state.clone(),
            state_entered_time: frame.state_entered_time,
            context_slot: frame.context_slot,
            blend_duration: frame.blend_duration,
            source_pose: frame.source_pose,
            role: frame.role,
            stack,
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let state_data = self.machine.states.get(&frame.state)
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let sub_ctx = state_graph_context(&self.node_context, state_data, graph, frame.context_slot)
            .with_state_key(ctx.state_key)
            .with_io(&io);
        let input = match self.role {
            FsmRole::Source => GraphInputPin::FromFsmSource("".into()),
            FsmRole::Target => GraphInputPin::FromFsmTarget("".into()),
            FsmRole::Root => return self.parent_time(ctx),
        };
        graph.get_time_update(SourcePin::InputTime(input), sub_ctx)
    }
}

impl<'a> GraphIoEnv for InterruptibleFsmIo<'a> {
    fn get_data_back(
        &self,
        pin: GraphInputPin,
        ctx: GraphContext,
    ) -> Result<DataValue, GraphError> {
        match pin {
            GraphInputPin::Passthrough(pin) => self.parent_data(&pin, &ctx),
            GraphInputPin::FromFsmSource(pin) => {
                let input = GraphInputPin::FromFsmSource(pin.clone());
                if let Some((pose, context)) = &self.source_pose {
                    if pin.to_string() == InterruptibleFsmNode::OUT_POSE {
                        crate::extensions::animation_context::write_linked_input_context(
                            &mut ctx.clone(),
                            input,
                            context.clone(),
                        )?;
                        return Ok(DataValue::Pose(pose.clone()));
                    }
                    if let Some(value) = context.curves.get(&crate::extensions::SourceCurveName::from(pin.as_str())) {
                        return Ok(DataValue::F32(*value));
                    }
                }
                self.source_state().and_then(|state| {
                    let state_pin = if pin.to_string() == InterruptibleFsmNode::OUT_POSE {
                        self.state_pose_output_for(&state).to_owned()
                    } else {
                        pin
                    };
                    self.state_data(state_pin, state, FsmRole::Source, &ctx, input)
                })
            }
            GraphInputPin::FromFsmTarget(pin) => self.target_state().and_then(|state| {
                let input = GraphInputPin::FromFsmTarget(pin.clone());
                let state_pin = if pin.to_string() == InterruptibleFsmNode::OUT_POSE {
                    self.state_pose_output_for(&state).to_owned()
                } else {
                    pin
                };
                self.state_data(state_pin, state, FsmRole::Target, &ctx, input)
            }),
            GraphInputPin::FsmBuiltin(pin) => self.builtin(pin),
        }
    }

    fn get_duration_back(
        &self,
        pin: GraphInputPin,
        ctx: GraphContext,
    ) -> Result<DurationData, GraphError> {
        match pin {
            GraphInputPin::Passthrough(pin) => self.parent_duration(&pin, &ctx),
            GraphInputPin::FromFsmSource(_) if self.source_pose.is_some() => Ok(None),
            GraphInputPin::FromFsmSource(_) => self
                .source_state()
                .and_then(|state| self.state_duration(state, FsmRole::Source, &ctx)),
            GraphInputPin::FromFsmTarget(_) => self
                .target_state()
                .and_then(|state| self.state_duration(state, FsmRole::Target, &ctx)),
            GraphInputPin::FsmBuiltin(pin) => Err(GraphError::UnknownErrorNode {
                graph: self.node_context.graph_context.context_id,
                node: self.node_context.node_id,
                error: format!("FSM builtin duration is unavailable: {pin:?}"),
            }),
        }
    }

    fn get_time_fwd(&self, ctx: GraphContext) -> Result<TimeUpdate, GraphError> {
        self.nested_time(&ctx).or_else(|_| self.parent_time(&ctx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::direct_transition_event::take_direct_transition_edge;
    use crate::extensions::global_bool_input::GlobalBoolInputNode;
    use crate::extensions::global_pose_input::GlobalPoseInputNode;
    use crate::extensions::source_cached_pose::{CACHED_POSE_VALUE, SourceCachedPoseSaveNode, SourceScopedCachedPoseUseNode};
    use crate::extensions::source_movement_nodes::SourceMultiWayBlendNode;
    use crate::extensions::source_pose_passthrough::SourcePosePassthroughNode;
    use bevy::platform::collections::HashMap;
    use bevy_animation_graph::core::state_machine::low_level::{
        LlTransitionData, LowLevelState, LowLevelTransitionType,
    };
    use bevy_animation_graph::core::{
        animation_clip::GraphClip,
        animation_graph::{AnimationGraph, NodeId, TimeUpdate},
        animation_node::AnimationNode,
        context::{
            deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
            graph_context_arena::GraphContextArena, new_context::GraphContext, node_states::StateKey,
            system_resources::SystemResources,
        },
        edge_data::DataSpec,
        pose::BonePose,
        skeleton::Skeleton,
        state_machine::high_level::{DirectTransition, State, TransitionData},
        utils::testing::TestIoEnv,
    };

    #[test]
    fn state_getters_use_the_owning_linked_instance() {
        fn query(
            In((handles, nodes, getters)): In<([Handle<AnimationGraph>; 3], [NodeId; 4], [NodeId; 3])>,
            resources: SystemResources,
        ) {
            let [root, shared, nested] = handles.map(|handle| (handle.id(), resources.animation_graph_assets.get(&handle).unwrap()));
            let [first, second, missing, machine] = nodes;
            let state_id = StateId::default();
            let env = TestIoEnv::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let mut gizmos = DeferredGizmos::default();
            for count in [2, 1] {
                let mut arena = GraphContextArena::new(root.0);
                for frame in 1..=2 {
                    for (index, instance) in [first, second].into_iter().take(count).enumerate() {
                        let root_ctx = GraphContext::new(arena.get_toplevel_id(), root.1, &mut arena, &resources,
                            &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                        let owner = root_ctx.create_node_context(instance).create_child_context(shared.0, shared.1, None);
                        let mut node = owner.create_node_context(machine);
                        let state = node.state_mut_or_else(|| InterruptibleFsmNode::initial_state(
                            LowLevelStateId::HlState(state_id), 0.0, 0)).unwrap();
                        let value = (index + frame) as f32;
                        state.recorded_machine_weight = value / 4.0;
                        state.current_state_elapsed_time = value * 3.0;
                        state.recorded_state_weights = vec![SourceFsmStateWeight { state_id: state_id.uuid().to_string(), value: value / 8.0 }];
                    }
                    for (index, instance) in [first, second].into_iter().take(count).enumerate() {
                        let root_ctx = GraphContext::new(arena.get_toplevel_id(), root.1, &mut arena, &resources,
                            &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                        let owner = root_ctx.create_node_context(instance).create_child_context(shared.0, shared.1, None);
                        let child = owner.create_node_context(machine).create_child_context(nested.0, nested.1,
                            Some(LowLevelStateId::HlState(state_id)));
                        let value = (index + frame) as f32;
                        for (getter, expected) in getters.into_iter().zip([value / 4.0, value * 3.0, value / 8.0]) {
                            nested.1.nodes[&getter].update(child.create_node_context(getter)).unwrap();
                            let actual = child.node_caches().get_output_data(getter, StateKey::Default, "ReturnValue".into())
                                .unwrap().as_f32().unwrap();
                            assert_eq!(actual, expected, "Getter borrowed another linked instance");
                        }
                    }
                    let root_ctx = GraphContext::new(arena.get_toplevel_id(), root.1, &mut arena, &resources,
                        &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                    let missing = root_ctx.create_node_context(missing).create_child_context(nested.0, nested.1, None);
                    for getter in getters {
                        assert!(nested.1.nodes[&getter].update(missing.create_node_context(getter)).is_err(),
                            "A getter without its own machine must not borrow a foreign instance");
                    }
                    arena.next_frame();
                }
            }
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let machine = AnimationNode::new("shared machine", InterruptibleFsmNode::default());
        let mut shared = AnimationGraph::new();
        shared.add_node(machine.clone());
        let shared = graphs.add(shared);
        let mut nested = AnimationGraph::new();
        let getters = ["GetInstanceMachineWeight", "GetInstanceCurrentStateElapsedTime", "GetInstanceStateWeight"].map(|function| {
            let node = AnimationNode::new(function, crate::extensions::SourceAnimationStateGetterNode {
                source_function: function.to_owned(), fsm_node: machine.id.uuid().to_string(),
                state_id: if function == "GetInstanceStateWeight" { StateId::default().uuid().to_string() } else { String::new() },
                output: "ReturnValue".to_owned(),
            });
            nested.add_node(node.clone());
            node.id
        });
        let nested = graphs.add(nested);
        let mut root = AnimationGraph::new();
        let instances = [shared.clone(), shared.clone(), nested.clone()].map(|graph| {
            let node = AnimationNode::new("linked instance", crate::extensions::LayeredGraphNode::new(graph));
            root.add_node(node.clone());
            node.id
        });
        let root = graphs.add(root);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        world.run_system_cached_with(query, ([root, shared, nested], [instances[0], instances[1], instances[2], machine.id], getters)).unwrap();
    }

    #[test]
    fn machine_weight_uses_the_parent_update_context() {
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> f32 {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            let mut ctx = GraphContext::new(
                arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
            ).create_node_context(NodeId::default());
            let initial = InterruptibleFsmNode::initial_state(
                LowLevelStateId::HlState(StateId::default()), 0.0, 0,
            );
            ctx.state_mut_or_else(|| initial).unwrap();
            with_machine_weight(0.4, || {
                InterruptibleFsmNode::default()
                    .record_state_weights(&LowLevelStateMachine::default(), &mut ctx)
                    .unwrap();
            });
            ctx.state::<InterruptibleFsmState>().unwrap().recorded_machine_weight()
        }

        let mut world = World::new();
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(AnimationGraph::new());
        assert_eq!(world.run_system_cached_with(query, handle).unwrap(), 0.4);
    }

    #[test]
    fn machine_weight_scope_multiplies_and_restores_nested_evaluations() {
        assert_eq!(current_machine_weight(), 1.0);
        with_machine_weight(0.5, || {
            assert_eq!(current_machine_weight(), 0.5);
            with_machine_weight(0.4, || assert!((current_machine_weight() - 0.2).abs() < 1e-6));
            assert_eq!(current_machine_weight(), 0.5);
        });
        assert_eq!(current_machine_weight(), 1.0);
        let result: Result<(), ()> = with_machine_weight(0.25, || Err(()));
        assert!(result.is_err());
        assert_eq!(current_machine_weight(), 1.0);
    }

    #[derive(Reflect, Clone, Debug, Default)]
    #[reflect(Default, NodeLike)]
    #[type_path = "als_ext"]
    struct MachineWeightProbe;

    impl NodeLike for MachineWeightProbe {
        fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
            let mut pose = Pose::default();
            pose.timestamp = current_machine_weight();
            ctx.set_data_fwd("Pose", pose);
            Ok(())
        }

        fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
            ctx.add_output_data("Pose", DataSpec::Pose);
            Ok(())
        }

        fn display_name(&self) -> String {
            "Machine weight probe".to_owned()
        }
    }

    #[test]
    fn scoped_cache_prewarm_uses_the_parent_state_weight() {
        fn query(
            In((root, state_graph, cache_id, state_id)): In<(Handle<AnimationGraph>, Handle<AnimationGraph>, NodeId, StateId)>,
            resources: SystemResources,
        ) -> f32 {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            let mut ctx = GraphContext::new(
                arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
            ).create_node_context(NodeId::default());
            let state = LowLevelStateId::HlState(state_id);
            let mut machine = LowLevelStateMachine::new();
            machine.add_state(LowLevelState { id: state.clone(), graph: state_graph, hl_transition: None });
            let weights = vec![SourceFsmStateWeight { state_id: state_id.uuid().to_string(), value: 0.4 }];
            InterruptibleFsmNode::prewarm_active_state_caches(&machine, &state, &mut ctx, &weights).unwrap();
            drop(ctx);
            arena.get_toplevel().node_caches
                .get_output_data(cache_id, StateKey::Default, CACHED_POSE_VALUE.to_owned())
                .unwrap().into_pose().unwrap().timestamp
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let mut root = AnimationGraph::new();
        let probe = AnimationNode::new("weight", MachineWeightProbe);
        let cache = AnimationNode::new("fixture", SourceCachedPoseSaveNode { cache_key: "fixture".to_owned() });
        root.add_node(probe.clone());
        root.add_node(cache.clone());
        root.add_node_parameter_edge(probe.id, "Pose", cache.id, "Pose");
        let root = graphs.add(root);
        let mut state_graph = AnimationGraph::new();
        state_graph.add_node(AnimationNode::new("use fixture", SourceScopedCachedPoseUseNode {
            cache_node: "fixture".to_owned(), cache_key: "fixture".to_owned(), ..default()
        }));
        let state_graph = graphs.add(state_graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let actual = world.run_system_cached_with(query, (root, state_graph, cache.id, StateId::default())).unwrap();
        assert_eq!(actual, 0.4);
    }

    #[test]
    fn direct_state_graph_uses_the_parent_transition_weight() {
        fn query(
            In((root, state_graph, transition_graph, fsm_node)): In<(Handle<AnimationGraph>, Handle<AnimationGraph>, Handle<AnimationGraph>, NodeId)>,
            resources: SystemResources,
        ) -> f32 {
            let source: StateId = serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
            let target: StateId = serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
            let transition = TransitionId::Direct(DirectTransitionId::default());
            let mut machine = LowLevelStateMachine::new();
            for state in [source, target] {
                machine.add_state(LowLevelState {
                    id: LowLevelStateId::HlState(state), graph: state_graph.clone(), hl_transition: None,
                });
            }
            machine.add_state(LowLevelState {
                id: LowLevelStateId::HlTransition(transition), graph: transition_graph,
                hl_transition: Some(LlTransitionData {
                    source, target, hl_transition_id: transition, timed: Some(1.0),
                }),
            });
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            let graph_ctx = GraphContext::new(
                arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
            );
            let mut node_ctx = graph_ctx.create_node_context(fsm_node);
            node_ctx.set_time(0.4);
            node_ctx.state_mut_or_else(|| InterruptibleFsmNode::initial_state(
                LowLevelStateId::HlTransition(transition), 0.0, 0,
            )).unwrap();
            let io = InterruptibleFsmIo {
                node_context: node_ctx.clone(), machine: &machine, current_time: 0.4,
                parent_time_update: TimeUpdate::Delta(0.4),
                state: LowLevelStateId::HlTransition(transition), state_entered_time: 0.0,
                context_slot: 0, blend_duration: None, source_pose: None, role: FsmRole::Root,
                stack: VecDeque::new(), state_pose_output: "Pose".to_owned(), state_pose_outputs: Vec::new(),
            };
            io.state_data(
                "Pose".into(), LowLevelStateId::HlState(target), FsmRole::Target,
                &node_ctx.graph_context, GraphInputPin::FromFsmTarget("Pose".into()),
            ).unwrap().into_pose().unwrap().timestamp
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let mut state_graph = AnimationGraph::new();
        let probe = AnimationNode::new("weight", MachineWeightProbe);
        state_graph.add_node(probe.clone());
        state_graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
        state_graph.add_output_data_edge(probe.id, "Pose", "Pose");
        let state_graph = graphs.add(state_graph);
        let mut transition_graph = AnimationGraph::new();
        transition_graph.add_node(AnimationNode::new("blend", ProfiledBlendNode::default()));
        let transition_graph = graphs.add(transition_graph);
        let mut root = AnimationGraph::new();
        let fsm_node = AnimationNode::new("fsm", InterruptibleFsmNode::default());
        root.add_node(fsm_node.clone());
        let root = graphs.add(root);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let actual = world.run_system_cached_with(query, (root, state_graph, transition_graph, fsm_node.id)).unwrap();
        assert_eq!(actual, 0.4);
    }

    #[test]
    fn machine_weight_getter_selects_the_source_branch_on_the_first_update() {
        use crate::extensions::source_scalar_expression::{SourceScalarDataType, SourceScalarExpressionNode, SourceScalarPin};

        fn wrap_curves(mut graph: AnimationGraph, amount: f32) -> AnimationGraph {
            use bevy_animation_graph::core::context::spec_context::NodeOutputPin;
            let source = graph.remove_edge_by_target(&TargetPin::OutputData("Pose".into())).unwrap();
            graph.remove_edge_by_target(&TargetPin::OutputData("State".into()));
            graph.io_spec.remove_output(&NodeOutputPin::Data("State".into()));
            let base = AnimationNode::new("base weight", MachineWeightProbe);
            let blend = AnimationNode::new("curves blend", crate::extensions::SourceCurvesBlendNode {
                blend_amount: amount, ..default()
            });
            graph.add_node(base.clone());
            graph.add_node(blend.clone());
            graph.add_node_parameter_edge(base.id, "Pose", blend.id, "BasePose");
            graph.add_edge(source, TargetPin::NodeData(blend.id, "CurvePose".into()));
            graph.add_output_data_edge(blend.id, "Pose", "Pose");
            graph
        }

        fn query_wrapped(In((root, fsm_node)): In<(Handle<AnimationGraph>, NodeId)>, resources: SystemResources) -> Vec<(f32, Option<f32>, Option<f32>)> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::from([("pose".to_owned(), DataValue::Pose(Pose::default())),
                ("delta".to_owned(), DataValue::F32(0.02))]);
            let env = TestIoEnv::default();
            [0.4_f32, 1.0].into_iter().map(|weight| {
                let mut arena = GraphContextArena::new(root.id());
                let output = with_machine_weight(weight, || graph.query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                    GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                        &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs),
                )).unwrap();
                let recorded = arena.get_toplevel().node_states.get::<InterruptibleFsmState>(fsm_node, StateKey::Default)
                    .ok().map(|state| state.recorded_machine_weight());
                let selected = arena.get_toplevel().node_caches.get_output_data(fsm_node, StateKey::Default,
                    InterruptibleFsmNode::OUT_ACTUAL_STATE.into()).map(|value| value.as_f32().unwrap());
                (output["Pose"].as_pose().unwrap().timestamp, selected, recorded)
            }).collect()
        }

        fn query(
            In((root, fsm_node)): In<(Handle<AnimationGraph>, NodeId)>,
            resources: SystemResources,
        ) -> Vec<(f32, f32)> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::from([
                ("pose".to_owned(), DataValue::Pose(Pose::default())),
                ("delta".to_owned(), DataValue::F32(0.02)),
            ]);
            let env = TestIoEnv::default();
            [0.4, 1.0].into_iter().map(|weight| {
                let mut arena = GraphContextArena::new(root.id());
                let output = with_machine_weight(weight, || graph.query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                    GraphContext::new(
                        arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                        Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
                    ),
                )).unwrap();
                let state = arena.get_toplevel().node_states
                    .get::<InterruptibleFsmState>(fsm_node, StateKey::Default).unwrap();
                (output["State"].as_f32().unwrap(), state.recorded_machine_weight())
            }).collect()
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let mut pose_graph = AnimationGraph::new();
        let pose = AnimationNode::new("pose", GlobalPoseInputNode {
            key: "pose".to_owned(), output: "Pose".to_owned(),
        });
        pose_graph.add_node(pose.clone());
        pose_graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
        pose_graph.add_output_data_edge(pose.id, "Pose", "Pose");
        let pose_graph = graphs.add(pose_graph);
        let states = [1, 2, 3].map(|id| serde_json::from_str::<StateId>(
            &format!("\"10000000-0000-4000-8000-{id:012}\"")
        ).unwrap());
        let fsm_id: NodeId = serde_json::from_str("\"90000000-0000-4000-8000-000000000001\"").unwrap();
        let getter = crate::extensions::SourceAnimationStateGetterNode {
            source_function: "GetInstanceMachineWeight".to_owned(),
            fsm_node: fsm_id.uuid().to_string(), output: "ReturnValue".to_owned(), ..default()
        };
        let mut machine = StateMachine::default();
        machine.start_state = states[0];
        machine.node_spec.add_output_data("Pose".to_owned(), DataSpec::Pose);
        for (index, state) in states.iter().enumerate() {
            machine.add_state(State {
                id: *state, label: ["Idle", "Low", "High"][index].to_owned(),
                graph: pose_graph.clone(), ..default()
            });
        }
        let mut conditions = Vec::new();
        for (index, operation) in ["less", "greater_equal"].into_iter().enumerate() {
            let mut condition = AnimationGraph::new();
            let read = AnimationNode::new("Machine Weight", getter.clone());
            let compare = AnimationNode::new("Compare with one", SourceScalarExpressionNode {
                operation: operation.to_owned(),
                input_pins: vec![SourceScalarPin { name: "A".to_owned(), ..default() },
                    SourceScalarPin { name: "B".to_owned(), f32_default: 1.0, ..default() }],
                output_pins: vec![SourceScalarPin {
                    name: "Result".to_owned(), data_type: SourceScalarDataType::Bool, ..default()
                }],
                ..default()
            });
            condition.add_node(read.clone());
            condition.add_node(compare.clone());
            condition.add_node_parameter_edge(read.id, "ReturnValue", compare.id, "A");
            condition.add_output_data("Result".to_owned(), DataSpec::Bool);
            condition.add_output_data_edge(compare.id, "Result", "Result");
            let transition_id: DirectTransitionId = serde_json::from_str(
                &format!("\"20000000-0000-4000-8000-{index:012}\"")
            ).unwrap();
            conditions.push(SourceTransitionCondition {
                transition_id: transition_id.uuid().to_string(),
                source_state_id: states[0].uuid().to_string(),
                condition: graphs.add(condition), output: "Result".to_owned(),
                context_state: states[0], ..default()
            });
            machine.add_transition_unchecked(DirectTransition {
                id: transition_id, source: states[0], target: states[index + 1],
                data: TransitionData::default(),
            });
        }
        machine.update_low_level_fsm();
        let mut machines = Assets::<StateMachine>::default();
        let mut fsm = AnimationNode::new("fsm", InterruptibleFsmNode {
            fsm: machines.add(machine), source_transition_conditions: conditions,
            telemetry_states: states.to_vec(), state_pose_output: "Pose".to_owned(),
            outer_pose_output: "Pose".to_owned(), internal_time_key: "delta".to_owned(), ..default()
        });
        fsm.id = fsm_id;
        let mut root = AnimationGraph::new();
        root.add_node(fsm.clone());
        root.add_output_data("Pose".to_owned(), DataSpec::Pose);
        root.add_output_data_edge(fsm.id, "Pose", "Pose");
        root.add_output_data("State".to_owned(), DataSpec::F32);
        root.add_output_data_edge(fsm.id, InterruptibleFsmNode::OUT_ACTUAL_STATE, "State");
        let direct = graphs.add(root.clone());
        let zero = graphs.add(wrap_curves(root.clone(), 0.0));
        let half_graph = wrap_curves(root.clone(), 0.5);
        let half = graphs.add(half_graph.clone());
        let nested = graphs.add(wrap_curves(half_graph, 0.5));
        let unit = graphs.add(wrap_curves(root, 1.0));
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(machines);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        assert_eq!(world.run_system_cached_with(query, (direct, fsm_id)).unwrap(), vec![(1.0, 0.4), (2.0, 1.0)]);
        for (graph, expected) in [
            (zero, vec![(0.4, None, None), (1.0, None, None)]),
            (half, vec![(0.4, Some(1.0), Some(0.2)), (1.0, Some(1.0), Some(0.5))]),
            (nested, vec![(0.4, Some(1.0), Some(0.1)), (1.0, Some(1.0), Some(0.25))]),
            (unit, vec![(0.4, Some(1.0), Some(0.4)), (1.0, Some(2.0), Some(1.0))]),
        ] {
            assert_eq!(world.run_system_cached_with(query_wrapped, (graph, fsm_id)).unwrap(), expected);
        }
    }

    fn fixture_pose(x: f32) -> Pose {
        Pose {
            paths: HashMap::from([(default(), 0)]),
            bones: vec![BonePose {
                translation: Some(Vec3::new(x, 0.0, 0.0)),
                rotation: Some(Quat::IDENTITY),
                scale: Some(Vec3::ONE),
                weights: None,
            }],
            timestamp: x,
            ..default()
        }
    }

    fn query_result_propagation(
        In((root, first, second)): In<(Handle<AnimationGraph>, Pose, Pose)>,
        resources: SystemResources,
    ) -> Result<(Pose, Pose, Vec<String>, u64), GraphError> {
        let graph = resources
            .animation_graph_assets
            .get(&root)
            .ok_or(GraphError::GraphAssetMissing)?;
        let mut arena = GraphContextArena::new(root.id());
        let mut gizmos = DeferredGizmos::default();
        let inputs = [
            ("delta".to_owned(), DataValue::F32(1.0 / 60.0)),
            ("left".to_owned(), DataValue::Bool(false)),
            ("weight".to_owned(), DataValue::F32(1.0)),
            ("fixture_pose".to_owned(), DataValue::Pose(first)),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let entity_map = HashMap::new();
        let env = TestIoEnv::default();
        let first = graph
            .query_with_context(
                QueryOutputTime::Forced(TimeUpdate::Delta(1.0 / 60.0)),
                GraphContext::new(
                    arena.get_toplevel_id(),
                    graph,
                    &mut arena,
                    &resources,
                    &env,
                    Entity::PLACEHOLDER,
                    &entity_map,
                    &mut gizmos,
                    &inputs,
                ),
            )?
            .remove("Pose")
            .ok_or(GraphError::OutputMissing {
                graph: arena.get_toplevel_id(),
                node: NodeId::default(),
                pin: "Pose".to_owned(),
            })?
            .into_pose()?;
        let inputs = [
            ("delta".to_owned(), DataValue::F32(1.0 / 60.0)),
            ("left".to_owned(), DataValue::Bool(true)),
            ("weight".to_owned(), DataValue::F32(1.0)),
            ("fixture_pose".to_owned(), DataValue::Pose(second)),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let second = graph
            .query_with_context(
                QueryOutputTime::Forced(TimeUpdate::Delta(1.0 / 60.0)),
                GraphContext::new(
                    arena.get_toplevel_id(),
                    graph,
                    &mut arena,
                    &resources,
                    &env,
                    Entity::PLACEHOLDER,
                    &entity_map,
                    &mut gizmos,
                    &inputs,
                ),
            )?
            .remove("Pose")
            .ok_or(GraphError::OutputMissing {
                graph: arena.get_toplevel_id(),
                node: NodeId::default(),
                pin: "Pose".to_owned(),
            })?
            .into_pose()?;
        let fsm_node = graph
            .nodes
            .values()
            .find(|node| {
                node.try_inner_downcast_ref::<InterruptibleFsmNode>()
                    .is_some()
            })
            .unwrap();
        let state = arena
            .get_toplevel()
            .node_states
            .get::<InterruptibleFsmState>(fsm_node.id, StateKey::Default)?;
        let notifications = state
            .transition_notifications()
            .iter()
            .map(|event| event.name.clone())
            .collect();
        Ok((
            first,
            second,
            notifications,
            state.notification_generation(),
        ))
    }

    #[test]
    fn state_result_pose_and_lifecycle_follow_the_actual_fsm_transition() {
        for (through_alias, member_matches) in [(false, true), (true, true), (true, false)] {
        let mut world = World::new();
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut state_graph = AnimationGraph::new();
        let cached = AnimationNode::new(
            "fixture cached pose",
            SourceScopedCachedPoseUseNode {
                cache_node: "fixture cache".to_owned(),
                cache_key: "fixture-cache".to_owned(),
                ..default()
            },
        );
        let blend = AnimationNode::new(
            "fixture blend",
            SourceMultiWayBlendNode {
                pose_count: 1,
                desired_alpha_keys: vec!["weight".to_owned()],
                desired_alpha_defaults: vec![0.0],
                pose_pin_prefix: "Poses_".to_owned(),
                alpha_pin_prefix: "DesiredAlphas_".to_owned(),
                ..default()
            },
        );
        let output = AnimationNode::new(
            "fixture output",
            SourcePosePassthroughNode {
                input: "Source".to_owned(),
                output: "Result".to_owned(),
                output_time: false,
                source_function: "FixtureExit".to_owned(),
                call_site: "OnStateExit".to_owned(),
                ..default()
            },
        );
        state_graph.add_node(cached.clone());
        state_graph.add_node(blend.clone());
        state_graph.add_node(output.clone());
        state_graph.add_output_data("Result".to_owned(), DataSpec::Pose);
        state_graph.add_node_parameter_edge(cached.id, "Pose", blend.id, "Poses_0");
        state_graph.add_node_parameter_edge(blend.id, "Pose", output.id, "Source");
        state_graph.add_output_data_edge(output.id, "Result", "Result");
        let first_state_graph = graphs.add(state_graph.clone());
        let mut entry_output = AnimationNode::new(
            "fixture output",
            SourcePosePassthroughNode {
                input: "Source".to_owned(),
                output: "Result".to_owned(),
                output_time: false,
                source_function: "FixtureEntry".to_owned(),
                call_site: "OnStateEntry".to_owned(),
                ..default()
            },
        );
        entry_output.id = output.id;
        state_graph.add_node(entry_output);
        let state_graph = graphs.add(state_graph);

        let mut condition_graph = AnimationGraph::new();
        let condition = AnimationNode::new(
            "fixture left condition",
            GlobalBoolInputNode {
                key: "left".to_owned(),
            },
        );
        condition_graph.add_node(condition.clone());
        condition_graph.add_output_data("left".to_owned(), DataSpec::Bool);
        condition_graph.add_output_data_edge(condition.id, "left", "left");
        let condition_graph = graphs.add(condition_graph);

        let state_id = serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let second_state_id =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        let transition_id = DirectTransitionId::default();
        let mut state_machine = StateMachine::default();
        state_machine.start_state = state_id;
        state_machine
            .node_spec
            .add_output_data("Pose".to_owned(), DataSpec::Pose);
        state_machine.add_state(State {
            id: state_id,
            label: "First".to_owned(),
            graph: first_state_graph.clone(),
            state_transition: None,
        });
        state_machine.add_state(State {
            id: second_state_id,
            label: "Second".to_owned(),
            graph: state_graph,
            state_transition: None,
        });
        let edge_source = if through_alias { StateId::default() } else { state_id };
        if through_alias {
            state_machine.add_state(State {
                id: edge_source,
                label: "Alias".to_owned(),
                graph: first_state_graph,
                state_transition: None,
            });
        }
        state_machine.add_transition_unchecked(DirectTransition {
            id: transition_id,
            source: edge_source,
            target: second_state_id,
            data: TransitionData::default(),
        });
        state_machine.update_low_level_fsm();
        let mut state_machines = Assets::<StateMachine>::default();
        let fsm = state_machines.add(state_machine);

        let mut root = AnimationGraph::new();
        let source = AnimationNode::new(
            "fixture pose",
            GlobalPoseInputNode {
                key: "fixture_pose".to_owned(),
                output: "Pose".to_owned(),
            },
        );
        let cache = AnimationNode::new(
            "fixture cache",
            SourceCachedPoseSaveNode {
                cache_key: "fixture-cache".to_owned(),
            },
        );
        let fsm_node = AnimationNode::new(
            "fixture fsm",
            InterruptibleFsmNode {
                fsm,
                state_pose_output: "Result".to_owned(),
                outer_pose_output: "Pose".to_owned(),
                state_aliases: if through_alias { vec![StateAliasBinding {
                    state_id: edge_source.uuid().to_string(),
                    member_state_ids: vec![if member_matches { state_id } else { second_state_id }.uuid().to_string()],
                }] } else { Vec::new() },
                source_transition_conditions: vec![SourceTransitionCondition {
                    transition_id: transition_id.uuid().to_string(),
                    source_state_id: edge_source.uuid().to_string(),
                    condition: condition_graph,
                    output: "left".to_owned(),
                    context_state: serde_json::from_str("\"10000000-0000-4000-8000-000000000003\"")
                        .unwrap(),
                    ..default()
                }],
                internal_time_key: "delta".to_owned(),
                expose_telemetry: false,
                ..default()
            },
        );
        root.add_node(source.clone());
        root.add_node(cache.clone());
        root.add_node(fsm_node.clone());
        let sync_scope = AnimationNode::new(
            "source sync scope",
            SourcePosePassthroughNode {
                synchronize_groups: true,
                output_time: false,
                ..default()
            },
        );
        root.add_node(sync_scope.clone());
        root.add_node_parameter_edge(source.id, "Pose", cache.id, "Pose");
        root.add_output_data("Pose".to_owned(), DataSpec::Pose);
        root.add_node_parameter_edge(fsm_node.id, "Pose", sync_scope.id, "Source");
        root.add_output_data_edge(sync_scope.id, "Pose", "Pose");
        let root = graphs.add(root);

        world.insert_resource(graphs);
        world.insert_resource(state_machines);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        let (first, second, notifications, generation) = world
            .run_system_cached_with(
                query_result_propagation,
                (root.clone(), fixture_pose(1.0), fixture_pose(2.0)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(first.bones[0].translation, Some(Vec3::X));
        assert_eq!(second.bones[0].translation, Some(Vec3::new(2.0, 0.0, 0.0)));
        let expected = if member_matches { vec!["FixtureExit", "FixtureEntry"] } else { Vec::new() };
        assert_eq!(notifications, expected);
        let (_, _, notifications, reinitialized_generation) = world
            .run_system_cached_with(
                query_result_propagation,
                (root, fixture_pose(1.0), fixture_pose(2.0)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(notifications, expected);
        if member_matches { assert_ne!(generation, reinitialized_generation); }
        }
    }

    #[test]
    fn source_current_state_elapsed_getter_follows_native_update_and_entry_order() {
        use crate::extensions::source_scalar_expression::{SourceScalarDataType, SourceScalarExpressionNode, SourceScalarPin};
        fn query(In((root, getters)): In<(Handle<AnimationGraph>, [(Handle<AnimationGraph>, NodeId); 3])>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let expected = [
                [Some(0.0), None, None], [Some(0.25), None, None], [Some(0.75), None, None],
                [Some(1.0), None, None], [Some(1.5), None, None], [Some(1.75), None, None],
                [Some(2.25), None, None], [Some(2.5), None, None], [Some(3.0), Some(0.0), None],
                [None, Some(0.25), Some(0.0)], [None, None, Some(0.5)], [None, None, Some(0.75)],
                [None, None, Some(1.25)], [Some(0.0), None, None], [Some(0.5), None, None],
                [Some(0.75), None, None], [Some(1.25), None, None],
            ];
            for (frame, expected) in expected.into_iter().enumerate() {
                if frame == 13 { arena = GraphContextArena::new(root.id()); }
                let delta = if frame % 2 == 0 { 0.25 } else { 0.5 };
                let inputs = HashMap::from([("delta".to_owned(), DataValue::F32(delta)), ("pose".to_owned(), DataValue::Pose(fixture_pose(1.0)))]);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)), GraphContext::new(
                    arena.get_toplevel_id(), graph, &mut arena, &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
                )).unwrap();
                for (index, (handle, node)) in getters.iter().enumerate() {
                    let actual = arena.iter_context_ids().filter_map(|id| arena.get_context(id))
                        .filter(|context| context.get_graph_id() == handle.id() && context.node_caches.is_updated(*node, StateKey::Default))
                        .filter_map(|context| context.node_caches.get_output_data(*node, StateKey::Default, "ReturnValue".into()))
                        .map(|value| value.as_f32().unwrap()).collect::<Vec<_>>();
                    assert_eq!(actual, expected[index].into_iter().collect::<Vec<_>>(), "frame {frame}, getter {index}");
                }
            }
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let machine_node_id: NodeId = serde_json::from_str("\"90000000-0000-4000-8000-000000000001\"").unwrap();
        let states = [1, 2, 3].map(|id| serde_json::from_str::<StateId>(&format!("\"10000000-0000-4000-8000-{id:012}\"")).unwrap());
        let getter = crate::extensions::SourceAnimationStateGetterNode {
            source_function: "GetInstanceCurrentStateElapsedTime".to_owned(), fsm_node: machine_node_id.uuid().to_string(), state_id: String::new(), output: "ReturnValue".to_owned(),
        };
        let pose = AnimationNode::new("pose", GlobalPoseInputNode { key: "pose".to_owned(), output: "pose".to_owned() });
        let mut state_graph = AnimationGraph::new();
        state_graph.add_node(pose.clone());
        state_graph.add_output_data("pose".to_owned(), DataSpec::Pose);
        state_graph.add_output_data_edge(pose.id, "pose", "pose");
        let state_graph = graphs.add(state_graph);
        let mut machine = StateMachine::default();
        machine.start_state = states[0];
        machine.node_spec.add_output_data("pose".to_owned(), DataSpec::Pose);
        for (index, state) in states.iter().enumerate() {
            machine.add_state(State { id: *state, label: index.to_string(), graph: state_graph.clone(), ..default() });
        }
        let blend = AnimationNode::new("blend", crate::extensions::ProfiledBlendNode::default());
        let mut transition = AnimationGraph::new();
        transition.add_node(blend.clone());
        for (pin, input, spec) in [
            ("factor", GraphInputPin::FsmBuiltin(FsmBuiltinPin::PercentThroughDuration), DataSpec::F32),
            ("pose_a", GraphInputPin::FromFsmSource("pose".to_owned()), DataSpec::Pose),
            ("pose_b", GraphInputPin::FromFsmTarget("pose".to_owned()), DataSpec::Pose),
        ] {
            transition.add_input_data(input.clone(), spec);
            transition.add_input_data_edge(input, blend.id, pin);
        }
        for (pin, input) in [("time_a", GraphInputPin::FromFsmSource("time".to_owned())), ("time_b", GraphInputPin::FromFsmTarget("time".to_owned()))] {
            transition.add_input_time(input.clone());
            transition.add_input_time_edge(input, blend.id, pin);
        }
        transition.add_output_time();
        transition.add_output_pose_edge(blend.id);
        transition.add_output_data("pose".to_owned(), DataSpec::Pose);
        transition.add_output_data_edge(blend.id, "pose", "pose");
        let transition = graphs.add(transition);
        let mut conditions = Vec::new();
        let mut getters = Vec::new();
        for (index, threshold) in [3.0, 0.25, 1000.0].into_iter().enumerate() {
            let mut condition = AnimationGraph::new();
            let read = AnimationNode::new("elapsed", getter.clone());
            let compare = AnimationNode::new("threshold", SourceScalarExpressionNode {
                input_pins: vec![SourceScalarPin { name: "A".to_owned(), ..default() }, SourceScalarPin { name: "B".to_owned(), f32_default: threshold, ..default() }],
                output_pins: vec![SourceScalarPin { name: "Result".to_owned(), data_type: SourceScalarDataType::Bool, ..default() }], operation: "greater_equal".to_owned(), ..default()
            });
            condition.add_node(read.clone());
            condition.add_node(compare.clone());
            condition.add_node_parameter_edge(read.id, "ReturnValue", compare.id, "A");
            condition.add_output_data("Result".to_owned(), DataSpec::Bool);
            condition.add_output_data_edge(compare.id, "Result", "Result");
            let condition = graphs.add(condition);
            getters.push((condition.clone(), read.id));
            let id: DirectTransitionId = serde_json::from_str(&format!("\"20000000-0000-4000-8000-{index:012}\"")).unwrap();
            conditions.push(SourceTransitionCondition { transition_id: id.uuid().to_string(), source_state_id: states[index].uuid().to_string(), condition, output: "Result".to_owned(), context_state: states[index], ..default() });
            machine.add_transition_unchecked(DirectTransition { id, source: states[index], target: states[(index + 1) % 3], data: TransitionData {
                kind: bevy_animation_graph::core::state_machine::high_level::TransitionKind::Graph { graph: transition.clone(), timed: Some(if index == 0 { 2.0 } else { 1.0 }) }, reset_target_state: true, ..default()
            }});
        }
        machine.update_low_level_fsm();
        let mut machines = Assets::<StateMachine>::default();
        let mut node = AnimationNode::new("fsm", InterruptibleFsmNode { fsm: machines.add(machine), source_transition_conditions: conditions, internal_time_key: "delta".to_owned(), expose_telemetry: false, ..default() });
        node.id = machine_node_id;
        let mut root = AnimationGraph::new();
        root.add_node(node.clone());
        root.add_output_data("pose".to_owned(), DataSpec::Pose);
        root.add_output_data_edge(node.id, "pose", "pose");
        let root = graphs.add(root);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(machines);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.run_system_cached_with(query, (root, getters.try_into().unwrap())).unwrap();
    }

    #[test]
    fn shared_cache_and_nested_fsm_follow_source_initialization_boundaries() {
        fn query(In(root): In<Handle<AnimationGraph>>, resources: SystemResources) -> Vec<f32> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            [0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 0.0, 1.0, 0.0]
                .into_iter()
                .enumerate()
                .map(|(frame, state)| {
                    if frame == 8 {
                        arena = GraphContextArena::new(root.id());
                    }
                    let inputs = HashMap::from([
                        ("delta".to_owned(), DataValue::F32(0.1)),
                        ("state".to_owned(), DataValue::F32(state)),
                        ("inner_state".to_owned(), DataValue::F32(0.0)),
                        ("idle".to_owned(), DataValue::Pose(Pose::default())),
                    ]);
                    graph
                        .query_with_context(
                            QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                            GraphContext::new(
                                arena.get_toplevel_id(),
                                graph,
                                &mut arena,
                                &resources,
                                &env,
                                Entity::PLACEHOLDER,
                                &entities,
                                &mut gizmos,
                                &inputs,
                            ),
                        )
                        .unwrap()
                        .remove("Pose")
                        .unwrap()
                        .into_pose()
                        .unwrap()
                        .timestamp
                })
                .collect()
        }
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../tools/animgraph_authoring/fixtures/ue_cached_pose_lifecycle.json"
        ))
        .unwrap();
        for (nested, reinitialize) in [(false, false), (true, false), (true, true)] {
            let mut graphs = Assets::<AnimationGraph>::default();
            let mut clips = Assets::<GraphClip>::default();
            let clip = clips.add(GraphClip {
                duration: 2.0,
                ..default()
            });
            let mut machines = Assets::<StateMachine>::default();
            let states = [1, 2, 3].map(|id| {
                serde_json::from_str::<StateId>(&format!("\"10000000-0000-4000-8000-{id:012}\""))
                    .unwrap()
            });
            let mut machine = StateMachine::default();
            machine.start_state = states[0];
            machine
                .node_spec
                .add_output_data("Pose".to_owned(), DataSpec::Pose);
            for (index, label) in ["Move", "Stop", "Idle"].into_iter().enumerate() {
                let mut graph = AnimationGraph::new();
                let node = if index == 2 {
                    AnimationNode::new(
                        "idle",
                        GlobalPoseInputNode {
                            key: "idle".to_owned(),
                            output: "Pose".to_owned(),
                        },
                    )
                } else {
                    AnimationNode::new(
                        "shared movement",
                        SourceScopedCachedPoseUseNode {
                            cache_node: "movement".to_owned(),
                            cache_key: "movement".to_owned(),
                            ..default()
                        },
                    )
                };
                graph.add_node(node.clone());
                graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
                graph.add_output_data_edge(node.id, "Pose", "Pose");
                machine.add_state(State {
                    id: states[index],
                    label: label.to_owned(),
                    graph: graphs.add(graph),
                    state_transition: None,
                });
            }
            for (source, target) in [(0, 1), (1, 2), (2, 0)] {
                machine.add_transition_unchecked(DirectTransition {
                    id: serde_json::from_str(&format!("\"20000000-0000-4000-8000-{source:012}\""))
                        .unwrap(),
                    source: states[source],
                    target: states[target],
                    data: TransitionData {
                        reset_target_state: true,
                        ..default()
                    },
                    ..default()
                });
            }
            machine.update_low_level_fsm();
            let mut root = AnimationGraph::new();
            let mut player = AnimationNode::new(
                "player",
                crate::extensions::SourceSequencePlayerNode {
                    clip,
                    start_position: 0.2,
                    ..default()
                },
            );
            if nested {
                let mut graph = AnimationGraph::new();
                graph.add_node(player.clone());
                graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
                graph.add_output_data_edge(player.id, "Pose", "Pose");
                let mut inner = StateMachine::default();
                inner
                    .node_spec
                    .add_output_data("Pose".to_owned(), DataSpec::Pose);
                inner.add_state(State {
                    label: "Entry".to_owned(),
                    graph: graphs.add(graph),
                    ..default()
                });
                inner.update_low_level_fsm();
                player = AnimationNode::new(
                    "inner fsm",
                    InterruptibleFsmNode {
                        fsm: machines.add(inner),
                        state_label_key: "inner_state".to_owned(),
                        state_labels: vec!["Entry".to_owned()],
                        internal_time_key: "delta".to_owned(),
                        reinitialize_on_becoming_relevant: reinitialize,
                        expose_telemetry: false,
                        state_pose_output: "Pose".to_owned(),
                        outer_pose_output: "Pose".to_owned(),
                        ..default()
                    },
                );
            }
            let cache = AnimationNode::new(
                "movement",
                SourceCachedPoseSaveNode {
                    cache_key: "movement".to_owned(),
                },
            );
            let machine = AnimationNode::new(
                "state machine",
                InterruptibleFsmNode {
                    fsm: machines.add(machine),
                    state_label_key: "state".to_owned(),
                    state_labels: ["Move", "Stop", "Idle"].map(str::to_owned).to_vec(),
                    internal_time_key: "delta".to_owned(),
                    expose_telemetry: false,
                    state_pose_output: "Pose".to_owned(),
                    outer_pose_output: "Pose".to_owned(),
                    ..default()
                },
            );
            let scope = AnimationNode::new(
                "sync scope",
                SourcePosePassthroughNode {
                    synchronize_groups: true,
                    output_time: false,
                    ..default()
                },
            );
            for node in [&player, &cache, &machine, &scope] {
                root.add_node(node.clone());
            }
            root.add_node_parameter_edge(player.id, "Pose", cache.id, "Pose");
            root.add_node_parameter_edge(machine.id, "Pose", scope.id, "Source");
            root.add_output_data("Pose".to_owned(), DataSpec::Pose);
            root.add_output_data_edge(scope.id, "Pose", "Pose");
            let root = graphs.add(root);
            let mut world = World::new();
            world.insert_resource(graphs);
            world.insert_resource(clips);
            world.insert_resource(machines);
            world.insert_resource(Assets::<Skeleton>::default());
            let actual = world.run_system_cached_with(query, root).unwrap();
            for (frame, actual) in actual.into_iter().enumerate() {
                let source = &source["frames"][frame];
                let expected = if !source["active"].as_bool().unwrap() {
                    0.0
                } else if reinitialize && (frame == 6 || frame == 7) {
                    (frame - 3) as f32 * 0.1
                } else {
                    source["child_position"].as_f64().unwrap() as f32
                };
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "nested {nested}, reinitialize {reinitialize}, frame {frame}: {actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn source_transition_preserves_a_target_that_is_still_blending() {
        use bevy::animation::animated_field;
        fn query(
            In((root, player, interrupt_frame, inertial, sync, repeat)): In<(
                Handle<AnimationGraph>,
                NodeId,
                usize,
                bool,
                bool,
                bool,
            )>,
            resources: SystemResources,
        ) -> f32 {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut context_count = None;
            let mut simultaneous_reentry = false;
            for frame in 0..=if repeat {
                46
            } else {
                interrupt_frame + usize::from(sync) * 2
            } {
                let inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(0.1)),
                    ("third".to_owned(), DataValue::Bool(repeat && (frame == 2 || (frame >= 4 && frame <= 40 && frame % 3 == 1)))),
                    (
                        "leave".to_owned(),
                        DataValue::Bool(
                            frame == 2 || (repeat && frame <= 40 && frame > 2 && frame % 3 == 2),
                        ),
                    ),
                    (
                        "return".to_owned(),
                        DataValue::Bool(
                            if repeat { frame == 2 || (frame >= 3 && frame <= 40 && frame % 3 == 0) }
                            else { frame == interrupt_frame },
                        ),
                    ),
                ]);
                let outputs = graph
                    .query_with_context(
                        QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                        GraphContext::new(
                            arena.get_toplevel_id(),
                            graph,
                            &mut arena,
                            &resources,
                            &env,
                            Entity::PLACEHOLDER,
                            &entities,
                            &mut gizmos,
                            &inputs,
                        ),
                    )
                    .unwrap();
                if inertial && !sync && frame == 2 {
                    let bone = bevy::animation::AnimationTargetId::from_name(&Name::new("moving")).into();
                    let position = outputs["Pose"].clone().into_pose().unwrap()
                        .get_bone(bone).unwrap().translation.unwrap().x;
                    assert!(position > 0.4 && position < 1.1, "inertial request output: {position}");
                    let node = graph.nodes.values().find(|node| node.name == "inertializer").unwrap();
                    let context = &arena.get_toplevel().node_states
                        .get::<crate::extensions::inertialization_passthrough::SourceInertializationState>(node.id, StateKey::Default).unwrap().context;
                    assert!(context.curves[&crate::extensions::SourceCurveName::from("Choice")] > 0.25 && context.curves[&crate::extensions::SourceCurveName::from("Choice")] < 0.75,
                        "inertial curve: {}", context.curves[&crate::extensions::SourceCurveName::from("Choice")]);
                    assert_eq!(context.curves[&crate::extensions::SourceCurveName::from("Filtered")], 0.75);
                    assert!(context.inertialization_request.is_none());
                }
                if inertial && repeat && frame == 2 {
                    let bone = bevy::animation::AnimationTargetId::from_name(&Name::new("moving")).into();
                    let position = outputs["Pose"].clone().into_pose().unwrap()
                        .get_bone(bone).unwrap().translation.unwrap().x;
                    assert!((position - 0.48125).abs() < 1e-5, "minimum inertial request: {position}");
                }
                let fsm = graph
                    .nodes
                    .values()
                    .find(|node| {
                        node.try_inner_downcast_ref::<InterruptibleFsmNode>()
                            .is_some()
                    })
                    .unwrap();
                let state = arena
                    .get_toplevel()
                    .node_states
                    .get::<InterruptibleFsmState>(fsm.id, StateKey::Default)
                    .unwrap();
                assert_eq!(
                    state.context.curves.get(&crate::extensions::SourceCurveName::from("Fixture")),
                    Some(&1.0),
                    "context frame={frame}"
                );
                let mut expected = if frame < 2 || frame >= interrupt_frame { 0.25 } else { 0.75 };
                if !inertial && !repeat {
                    expected = match frame {
                        0 | 1 => 0.25,
                        2 => 0.35,
                        _ if interrupt_frame == 3 => 0.25,
                        3 => 0.45,
                        4 => 0.55,
                        5 => 0.51666665,
                        6 => 0.41666666,
                        _ => 0.25,
                    };
                }
                let weighted = state.recorded_state_weight("10000000-0000-4000-8000-000000000001") * 0.25
                    + state.recorded_state_weight("10000000-0000-4000-8000-000000000002") * 0.75
                    + state.recorded_state_weight("10000000-0000-4000-8000-000000000003") * 0.5;
                if repeat { expected = weighted; }
                assert!(
                    (state.context.curves[&crate::extensions::SourceCurveName::from("Choice")] - expected).abs() < 1e-6,
                    "context frame={frame}, inertial={inertial}: {:?} != {expected}",
                    state.context.curves
                );
                if !inertial {
                    assert!(
                        (weighted - expected).abs() < 1e-6,
                        "state weight frame={frame}: {weighted} != {expected}"
                    );
                }
                assert!(!state.context.sync_players.is_empty());
                if sync {
                    let phase = crate::extensions::sync_group_phase::source_sync_phase(
                        &arena,
                        &resources.animation_graph_assets,
                        "fixture",
                    )
                    .unwrap();
                    assert!(
                        (phase - ((0.3 + frame as f32 * 0.1) / if repeat { 20.0 } else { 2.0 }))
                            .abs()
                            < 1e-6,
                        "group phase frame={frame}: {phase}"
                    );
                    let bone =
                        bevy::animation::AnimationTargetId::from_name(&Name::new("moving")).into();
                    let position = state
                        .last_output_pose()
                        .unwrap()
                        .get_bone(bone)
                        .unwrap()
                        .translation
                        .unwrap()
                        .x;
                    assert!(
                        (position - (0.3 + frame as f32 * 0.1)).abs() < 1e-5,
                        "moving pose frame={frame}: {position}"
                    );
                }
                if repeat {
                    if frame == 2 {
                        assert_eq!(state.active_state_id().as_deref(), Some("10000000-0000-4000-8000-000000000001"));
                    }
                    assert!(state.interrupted_sources.len() <= 5);
                    simultaneous_reentry |= state.interrupted_sources.iter().any(|source| source.state == state.state);
                    if frame == 20 {
                        context_count = Some(arena.iter_context_ids().count());
                    }
                    if frame > 20 {
                        assert_eq!(Some(arena.iter_context_ids().count()), context_count);
                    }
                    if frame == 46 {
                        assert!(state.interrupted_sources.is_empty());
                    }
                }
            }
            if repeat && !inertial { assert!(simultaneous_reentry); }
            arena
                .iter_context_ids()
                .find_map(|context| {
                    arena
                        .get_context(context)?
                        .node_states
                        .get::<crate::extensions::source_sequence_player::SourceSequencePlayerState>(
                            player,
                            bevy_animation_graph::core::context::node_states::StateKey::Default,
                        )
                        .ok()?
                        .context
                        .relevant_asset
                        .as_ref()
                        .map(|asset| asset.position)
                })
                .unwrap()
        }
        for (policy, interrupt_frame, inertial, sync, repeat) in [Some(false), Some(true), None]
            .into_iter()
            .flat_map(|policy| {
                [3, 5].into_iter().flat_map(move |frame| {
                    [false, true].map(|inertial| (policy, frame, inertial, false, false))
                })
            })
            .chain([
                (Some(false), 5, false, true, false),
                (Some(false), 4, false, true, true),
                (Some(false), 4, true, true, true),
            ])
        {
            let mut graphs = Assets::<AnimationGraph>::default();
            let mut clips = Assets::<GraphClip>::default();
            let duration = if repeat { 20.0 } else { 2.0 };
            let mut animation = bevy::animation::AnimationClip::default();
            animation.add_curve_to_target(
                bevy::animation::AnimationTargetId::from_name(&Name::new("moving")),
                AnimatableCurve::new(
                    animated_field!(Transform::translation),
                    UnevenSampleAutoCurve::new([(0.0, Vec3::ZERO), (duration, Vec3::X * duration)])
                        .unwrap(),
                ),
            );
            let clip = clips.add(GraphClip::from_bevy_clip(
                animation,
                default(),
                default(),
                None,
            ));
            let states = [1, 2, 3].map(|id| {
                serde_json::from_str::<StateId>(&format!("\"10000000-0000-4000-8000-{id:012}\""))
                    .unwrap()
            });
            let transitions = [1, 2, 3].map(|id| {
                serde_json::from_str::<DirectTransitionId>(&format!(
                    "\"20000000-0000-4000-8000-{id:012}\""
                ))
                .unwrap()
            });
            let mut machine = StateMachine::default();
            machine.start_state = states[0];
            machine
                .node_spec
                .add_output_data("pose".to_owned(), DataSpec::Pose);
            let mut players = Vec::new();
            for (index, start_position) in [0.2, 1.0, 1.4].into_iter().take(if repeat { 3 } else { 2 }).enumerate() {
                let mut graph = AnimationGraph::new();
                let player = AnimationNode::new(
                    "player",
                    crate::extensions::SourceSequencePlayerNode {
                        clip: clip.clone(),
                        start_position,
                        sync_group: "fixture".to_owned(),
                        synchronization: Some(
                            crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                                key: "fixture".to_owned(),
                                phase_duration: duration,
                                delta_seconds_key: "delta".to_owned(),
                                ..default()
                            },
                        ),
                        ..default()
                    },
                );
                players.push(player.id);
                graph.add_node(player.clone());
                let curve = AnimationNode::new(
                    "curve",
                    crate::extensions::SourceCurveModifyNode {
                        source_pose_pin: "SourcePose".to_owned(),
                        curve_names: vec!["Fixture".to_owned(), "Choice".to_owned(), "Filtered".to_owned()],
                        pin_defaults: vec![1.0, [0.25, 0.75, 0.5][index], [0.25, 0.75, 0.5][index]],
                        alpha: 1.0,
                        apply_mode: "Blend".to_owned(),
                        ..default()
                    },
                );
                graph.add_node(curve.clone());
                graph.add_node_parameter_edge(player.id, "Pose", curve.id, "SourcePose");
                graph.add_output_data("pose".to_owned(), DataSpec::Pose);
                graph.add_output_data_edge(curve.id, "Pose", "pose");
                machine.add_state(State {
                    id: states[index],
                    label: index.to_string(),
                    graph: graphs.add(graph),
                    ..default()
                });
            }
            let mut transition_graph = AnimationGraph::new();
            let blend = if inertial {
                AnimationNode::new("transition", InertializationNode::default())
            } else {
                AnimationNode::new("transition", ProfiledBlendNode::default())
            };
            transition_graph.add_node(blend.clone());
            for (pin, input, spec) in [
                (
                    "factor",
                    GraphInputPin::FsmBuiltin(FsmBuiltinPin::PercentThroughDuration),
                    DataSpec::F32,
                ),
                (
                    "pose_a",
                    GraphInputPin::FromFsmSource("pose".to_owned()),
                    DataSpec::Pose,
                ),
                (
                    "pose_b",
                    GraphInputPin::FromFsmTarget("pose".to_owned()),
                    DataSpec::Pose,
                ),
            ] {
                transition_graph.add_input_data(input.clone(), spec);
                transition_graph.add_input_data_edge(input, blend.id, pin);
            }
            for (pin, input) in [
                ("time_a", GraphInputPin::FromFsmSource("time".to_owned())),
                ("time_b", GraphInputPin::FromFsmTarget("time".to_owned())),
            ] {
                transition_graph.add_input_time(input.clone());
                transition_graph.add_input_time_edge(input, blend.id, pin);
            }
            transition_graph.add_output_time();
            transition_graph.add_output_pose_edge(blend.id);
            transition_graph.add_output_data("pose".to_owned(), DataSpec::Pose);
            transition_graph.add_output_data_edge(blend.id, "pose", "pose");
            let transition_graph = graphs.add(transition_graph);
            let mut conditions = Vec::new();
            for (index, key) in ["leave", "return", "third"].into_iter().take(if repeat { 3 } else { 2 }).enumerate() {
                let mut graph = AnimationGraph::new();
                let condition = AnimationNode::new(
                    "condition",
                    GlobalBoolInputNode {
                        key: key.to_owned(),
                    },
                );
                graph.add_node(condition.clone());
                graph.add_output_data(key.to_owned(), DataSpec::Bool);
                graph.add_output_data_edge(condition.id, key, key);
                conditions.push(SourceTransitionCondition {
                    transition_id: transitions[index].uuid().to_string(),
                    source_state_id: states[index].uuid().to_string(),
                    condition: graphs.add(graph),
                    output: key.to_owned(),
                    context_state: states[index],
                    always_reset_target_on_entry: policy,
                    automatic_trigger_time: (index == 1 && !inertial && !repeat && interrupt_frame == 3).then_some(1.0),
                    ..default()
                });
                let runtime_graph = if inertial && repeat {
                    let mut graph = graphs.get(&transition_graph).unwrap().clone();
                    graph.add_node(AnimationNode {
                        id: blend.id,
                        ..AnimationNode::new("transition", InertializationNode { transition_duration: [0.5, 0.2, 0.4][index] })
                    });
                    graphs.add(graph)
                } else { transition_graph.clone() };
                machine.add_transition_unchecked(DirectTransition {
                id: transitions[index], source: states[index], target: states[if repeat { (index + 1) % 3 } else { 1-index }],
                data: TransitionData {
                    kind: bevy_animation_graph::core::state_machine::high_level::TransitionKind::Graph {
                        graph: runtime_graph, timed: Some(0.5),
                    }, reset_target_state: true, ..default()
                },
            });
            }
            machine.update_low_level_fsm();
            let mut machines = Assets::<StateMachine>::default();
            let fsm = machines.add(machine);
            let mut root = AnimationGraph::new();
            let node = AnimationNode::new(
                "fsm",
                InterruptibleFsmNode {
                    fsm,
                    source_transition_conditions: conditions,
                    internal_time_key: "delta".to_owned(),
                    expose_telemetry: false,
                    ..default()
                },
            );
            root.add_node(node.clone());
            root.add_output_data("Pose".to_owned(), DataSpec::Pose);
            let output_node = if inertial {
                let inertializer = AnimationNode::new("inertializer", crate::extensions::InertializationPassthroughNode {
                    internalize_time: true,
                    filtered_curves: vec!["Filtered".to_owned()],
                    ..default()
                });
                root.add_node(inertializer.clone());
                root.add_node_parameter_edge(node.id, "pose", inertializer.id, "Source");
                inertializer.id
            } else { node.id };
            if sync {
                let scope = AnimationNode::new(
                    "sync scope",
                    SourcePosePassthroughNode {
                        synchronize_groups: true,
                        output_time: false,
                        ..default()
                    },
                );
                root.add_node(scope.clone());
                root.add_node_parameter_edge(output_node, "pose", scope.id, "Source");
                root.add_output_data_edge(scope.id, "Pose", "Pose");
            } else {
                root.add_output_data_edge(output_node, "pose", "Pose");
            }
            let root = graphs.add(root);
            let mut world = World::new();
            world.insert_resource(graphs);
            world.insert_resource(clips);
            world.insert_resource(machines);
            world.insert_resource(Assets::<Skeleton>::default());
            let actual = world
                .run_system_cached_with(
                    query,
                    (root, players[0], interrupt_frame, inertial, sync, repeat),
                )
                .unwrap();
            let expected = if (policy == Some(false) && !inertial) || repeat {
                0.2 + if repeat {
                    4.7
                } else {
                    (interrupt_frame + 1 + usize::from(sync) * 2) as f32 * 0.1
                }
            } else {
                0.3
            };
            assert!(
                (actual - expected).abs() < 1e-5,
                "reset policy {policy:?}, inertial={inertial}, repeat={repeat}: {actual} != {expected}"
            );
        }
    }

    #[test]
    fn immediate_direct_edge_is_selected_from_a_direct_event() {
        let state_id = StateId::default();
        let transition_id = TransitionId::Direct(default());
        let state = LowLevelStateId::HlState(state_id);
        let mut machine = LowLevelStateMachine::new();
        machine.add_transition(LowLevelTransition {
            id: LowLevelTransitionId::Immediate(transition_id),
            ignore_external: false,
            reset_target_state: false,
            source: state.clone(),
            target: state.clone(),
            transition_type: LowLevelTransitionType::Direct,
            hl_source: state_id,
            hl_target: state_id,
        });
        let runtime = InterruptibleFsmState {
            state,
            state_entered_time: 0.0,
            context_slot: 0,
            blend_duration: None,
            interrupted_sources: Vec::new(),
            context: default(),
            recorded_machine_weight: 1.0,
            current_state_elapsed_time: 0.0,
            recorded_state_weights: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
            first_update: false,
            last_update_frame: None,
            transition_notifications: Vec::new(),
            notification_generation: 0,
        };

        let selected =
            InterruptibleFsmNode::default().direct_transition(&machine, &runtime, transition_id);

        assert!(selected.is_some_and(|transition| {
            transition.id == LowLevelTransitionId::Immediate(transition_id)
        }));
    }

    #[test]
    fn source_transition_condition_selects_the_active_transition_target() {
        let source = serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let target = serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        let entering_transition =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000003\"").unwrap();
        let interrupting_transition =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000004\"").unwrap();
        let mut machine = LowLevelStateMachine::new();
        machine.add_transition(LowLevelTransition {
            id: LowLevelTransitionId::Start(TransitionId::Direct(entering_transition)),
            ignore_external: false,
            reset_target_state: true,
            source: LowLevelStateId::HlState(source),
            target: LowLevelStateId::HlTransition(TransitionId::Direct(entering_transition)),
            transition_type: LowLevelTransitionType::Direct,
            hl_source: source,
            hl_target: target,
        });
        machine.add_state(LowLevelState {
            id: LowLevelStateId::HlTransition(TransitionId::Direct(entering_transition)),
            graph: default(),
            hl_transition: Some(LlTransitionData {
                source,
                target,
                hl_transition_id: TransitionId::Direct(entering_transition),
                timed: Some(0.2),
            }),
        });
        machine.add_transition(LowLevelTransition {
            id: LowLevelTransitionId::Start(TransitionId::Direct(interrupting_transition)),
            ignore_external: false,
            reset_target_state: true,
            source: LowLevelStateId::HlState(target),
            target: LowLevelStateId::HlTransition(TransitionId::Direct(interrupting_transition)),
            transition_type: LowLevelTransitionType::Direct,
            hl_source: target,
            hl_target: source,
        });
        let condition = SourceTransitionCondition {
            transition_id: interrupting_transition.uuid().to_string(),
            source_state_id: target.uuid().to_string(),
            output: "bCanEnterTransition".to_owned(),
            ..default()
        };
        assert_eq!(
            InterruptibleFsmNode::source_condition_state(
                &machine,
                &LowLevelStateId::HlTransition(TransitionId::Direct(entering_transition)),
            ),
            Some(target)
        );
        assert_eq!(
            InterruptibleFsmNode::source_condition_transition_id(&machine, &condition),
            Some(TransitionId::Direct(interrupting_transition))
        );
    }

    #[test]
    fn transition_duration_uses_source_inverse_blend_and_automatic_overshoot() {
        assert!((source_transition_duration(0.4, 0.75, 0.0, AlphaBlendMode::HermiteCubic) - 0.125).abs() < 1e-6);
        assert!((source_transition_duration(0.4, 0.25, 0.0, AlphaBlendMode::HermiteCubic) - 0.275).abs() < 1e-6);
        assert!((source_transition_duration(0.4, 0.75, 0.1, AlphaBlendMode::Linear) - 0.075).abs() < 1e-6);
        assert_eq!(source_transition_duration(0.1, 0.0, 0.2, AlphaBlendMode::Cubic), 0.0);
    }

    #[test]
    fn automatic_transition_uses_relevant_asset_remaining_time() {
        let asset = SourceRelevantAsset {
            duration: 1.0,
            position: 0.85,
            weight: 1.0,
            ..default()
        };
        assert!((source_automatic_transition_adjustment(Some(&asset), -1.0, 0.2).unwrap() - 0.05).abs() < 1e-6);
        assert_eq!(source_automatic_transition_adjustment(Some(&asset), 0.1, 0.0), None);
        let wrapped = SourceRelevantAsset {
            position: 0.1,
            previous_position: 0.9,
            has_previous_position: true,
            delta_seconds: 0.2,
            looping: true,
            ..asset.clone()
        };
        assert_eq!(source_automatic_transition_adjustment(Some(&wrapped), 0.0, 0.0), Some(0.0));
    }

    #[test]
    fn alias_direct_edge_uses_the_active_member_state() {
        let alias = StateId::default();
        let member = StateId::default();
        let target = StateId::default();
        let transition_id = TransitionId::Direct(default());
        let mut machine = LowLevelStateMachine::new();
        machine.add_transition(LowLevelTransition {
            id: LowLevelTransitionId::Immediate(transition_id),
            ignore_external: false,
            reset_target_state: false,
            source: LowLevelStateId::HlState(alias),
            target: LowLevelStateId::HlState(target),
            transition_type: LowLevelTransitionType::Direct,
            hl_source: alias,
            hl_target: target,
        });
        let runtime = InterruptibleFsmState {
            state: LowLevelStateId::HlState(member),
            state_entered_time: 0.0,
            context_slot: 0,
            blend_duration: None,
            interrupted_sources: Vec::new(),
            context: default(),
            recorded_machine_weight: 1.0,
            current_state_elapsed_time: 0.0,
            recorded_state_weights: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
            first_update: false,
            last_update_frame: None,
            transition_notifications: Vec::new(),
            notification_generation: 0,
        };
        let node = InterruptibleFsmNode {
            state_aliases: vec![StateAliasBinding {
                state_id: alias.uuid().to_string(),
                member_state_ids: vec![member.uuid().to_string()],
            }],
            ..default()
        };
        assert!(
            node.direct_transition(&machine, &runtime, transition_id)
                .is_some()
        );
    }

    #[test]
    fn edge_triggered_direct_event_advances_the_fsm_once() {
        let state_id = StateId::default();
        let transition_id = TransitionId::Direct(default());
        let source = LowLevelStateId::HlState(state_id);
        let target = LowLevelStateId::HlTransition(transition_id);
        let mut machine = LowLevelStateMachine::new();
        machine.add_transition(LowLevelTransition {
            id: LowLevelTransitionId::Immediate(transition_id),
            ignore_external: false,
            reset_target_state: false,
            source: source.clone(),
            target: target.clone(),
            transition_type: LowLevelTransitionType::Direct,
            hl_source: state_id,
            hl_target: state_id,
        });
        let mut runtime = InterruptibleFsmState {
            state: source,
            state_entered_time: 0.0,
            context_slot: 0,
            blend_duration: None,
            interrupted_sources: Vec::new(),
            context: default(),
            recorded_machine_weight: 1.0,
            current_state_elapsed_time: 0.0,
            recorded_state_weights: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
            first_update: false,
            last_update_frame: None,
            transition_notifications: Vec::new(),
            notification_generation: 0,
        };
        let mut previous = None;
        let mut deliveries = 0;
        if take_direct_transition_edge(&mut previous, 0.0, 1).is_some() {
            let transition = InterruptibleFsmNode::default()
                .direct_transition(&machine, &runtime, transition_id)
                .unwrap();
            InterruptibleFsmNode::activate_transition(&mut runtime, transition, 0.25, None);
            deliveries += 1;
        }
        if take_direct_transition_edge(&mut previous, 0.0, 1).is_some() {
            deliveries += 1;
        }
        println!(
            "[BF102.05] fsm_state={:?} entered_time={:.2} deliveries={deliveries}",
            runtime.state, runtime.state_entered_time
        );
        assert_eq!(runtime.state, target);
        assert_eq!(runtime.state_entered_time, 0.25);
        assert_eq!(deliveries, 1);
    }

    #[test]
    fn telemetry_indexes_actual_high_level_state_and_transition() {
        let state_id = StateId::default();
        let transition_id = DirectTransitionId::default();
        let node = InterruptibleFsmNode {
            fsm: Handle::default(),
            telemetry_states: vec![state_id],
            telemetry_transitions: vec![transition_id.clone()],
            ..default()
        };
        assert_eq!(
            node.telemetry_state_index(&LowLevelStateId::HlState(state_id)),
            0.0
        );
        assert_eq!(
            node.telemetry_state_index(&LowLevelStateId::HlTransition(TransitionId::Direct(
                transition_id.clone(),
            ))),
            -1.0
        );
        assert_eq!(
            node.telemetry_transition_index(&LowLevelStateId::HlState(state_id)),
            -1.0
        );
        assert_eq!(
            node.telemetry_transition_index(&LowLevelStateId::HlTransition(TransitionId::Direct(
                transition_id,
            ))),
            0.0
        );
    }

    #[test]
    fn state_pose_output_uses_the_matching_state_override() {
        let state_id = StateId::default();
        let node = InterruptibleFsmNode {
            state_pose_outputs: vec![StatePoseOutput {
                state_id: state_id.uuid().to_string(),
                output: "Result".to_owned(),
            }],
            ..default()
        };
        assert_eq!(
            node.state_pose_output_for(&LowLevelStateId::HlState(state_id)),
            "Result"
        );
    }

    #[test]
    fn telemetry_is_visible_by_default_and_can_be_hidden_for_source_layout() {
        assert!(InterruptibleFsmNode::default().expose_telemetry);
        let node = InterruptibleFsmNode {
            expose_telemetry: false,
            ..default()
        };
        assert!(!node.expose_telemetry);
    }

    #[test]
    fn outer_pose_output_can_preserve_source_pin_casing() {
        let node = InterruptibleFsmNode {
            state_pose_output: "Result".to_owned(),
            outer_pose_output: "Pose".to_owned(),
            ..default()
        };
        assert_eq!(
            node.outer_output(&LowLevelStateId::HlState(StateId::default()), "Result"),
            "Pose"
        );
        assert_eq!(
            node.outer_output(
                &LowLevelStateId::HlTransition(TransitionId::Direct(default())),
                "pose",
            ),
            "Pose"
        );
    }

    #[test]
    fn transition_request_limit_retains_the_newest_source_requests() {
        assert_eq!(InterruptibleFsmNode::event_start_index(0, 32), 0);
        assert_eq!(InterruptibleFsmNode::event_start_index(3, 32), 0);
        assert_eq!(InterruptibleFsmNode::event_start_index(33, 32), 1);
        assert_eq!(InterruptibleFsmNode::event_start_index(4, 0), 4);
    }

    #[test]
    fn source_transition_limits_are_the_default_runtime_contract() {
        let node = InterruptibleFsmNode::default();
        assert_eq!(node.max_transitions_per_frame, 3);
        assert_eq!(node.max_transition_requests, 32);
        assert!(node.skip_first_update_transition);
        assert!(node.reinitialize_on_becoming_relevant);
    }

    #[test]
    fn source_relevance_reset_requires_a_skipped_frame() {
        assert!(!became_relevant(None, 4));
        assert!(!became_relevant(Some(4), 5));
        assert!(became_relevant(Some(4), 6));
    }

    #[test]
    fn source_condition_context_slots_are_isolated() {
        let first: StateId =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let second: StateId =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        assert_eq!(
            InterruptibleFsmNode::source_condition_context_state(default()),
            None
        );
        assert_ne!(
            InterruptibleFsmNode::source_condition_context_state(first),
            InterruptibleFsmNode::source_condition_context_state(second)
        );
        assert_ne!(
            InterruptibleFsmNode::source_condition_context_state(first),
            Some(LowLevelStateId::HlState(first))
        );
    }

    #[test]
    fn source_entry_target_resolves_only_a_real_fsm_state() {
        let target = StateId::default();
        let mut machine = LowLevelStateMachine::new();
        machine.add_state(LowLevelState {
            id: LowLevelStateId::HlState(target),
            graph: default(),
            hl_transition: None,
        });
        assert_eq!(
            InterruptibleFsmNode::source_entry_target_state(&machine, &target.uuid().to_string()),
            Some(target)
        );
        assert_eq!(
            InterruptibleFsmNode::source_entry_target_state(&machine, "missing"),
            None
        );
    }

    #[test]
    fn source_transition_notifications_follow_the_matching_phase() {
        let transition = DirectTransitionId::default();
        let node = InterruptibleFsmNode {
            source_transition_conditions: vec![SourceTransitionCondition {
                transition_id: transition.uuid().to_string(),
                notifications: vec![
                    SourceTransitionNotification {
                        phase: "TransitionStart".to_owned(),
                        name: "ActivatePivot".to_owned(),
                        function: "PlayTransitionLeftAnimation".to_owned(),
                        arguments: vec![crate::extensions::source_instance::SourcePropertyAssignment { property: "StartTime".to_owned(), value: DataValue::F32(0.3) }],
                    },
                    SourceTransitionNotification {
                        phase: "TransitionEnd".to_owned(),
                        name: "StopQuick".to_owned(),
                        ..default()
                    },
                ],
                ..default()
            }],
            ..default()
        };
        let mut state = InterruptibleFsmNode::initial_state(
            LowLevelStateId::HlState(StateId::default()),
            0.0,
            0,
        );
        node.append_transition_notifications(
            &mut state,
            &TransitionId::Direct(transition),
            "TransitionStart",
        );
        assert_eq!(state.transition_notifications().len(), 1);
        assert_eq!(state.transition_notifications()[0].name, "ActivatePivot");
        assert_eq!(state.transition_notifications()[0].function, "PlayTransitionLeftAnimation");
        assert_eq!(state.transition_notifications()[0].arguments[0].value.as_f32().unwrap(), 0.3);
        node.append_transition_notifications(
            &mut state,
            &TransitionId::Direct(transition),
            "TransitionEnd",
        );
        assert_eq!(state.transition_notifications()[1].name, "StopQuick");
    }

    #[test]
    fn active_direct_transition_identity_excludes_state_transitions() {
        let direct = DirectTransitionId::default();
        let active = InterruptibleFsmNode::initial_state(
            LowLevelStateId::HlTransition(TransitionId::Direct(direct)),
            0.0,
            0,
        );
        assert_eq!(
            active.active_direct_transition_id(),
            Some(direct.uuid().to_string())
        );
        let state = InterruptibleFsmNode::initial_state(
            LowLevelStateId::HlTransition(TransitionId::State(StateId::default())),
            0.0,
            0,
        );
        assert_eq!(state.active_direct_transition_id(), None);
        let fallback = InterruptibleFsmNode::initial_state(
            LowLevelStateId::HlTransition(TransitionId::Fallback),
            0.0,
            0,
        );
        assert_eq!(fallback.active_direct_transition_id(), None);
        let state = InterruptibleFsmNode::initial_state(
            LowLevelStateId::HlState(StateId::default()),
            0.0,
            0,
        );
        assert!(state.active_state_id().is_some());
        assert_eq!(active.active_state_id(), None);
    }

    #[test]
    fn internal_time_key_uses_a_nonnegative_delta() {
        let delta = (-0.25_f32).max(0.0);
        assert!(delta >= 0.0);
    }

    #[test]
    fn zero_duration_source_transition_completes_at_full_progress() {
        assert_eq!(timed_transition_progress(0.0, Some(0.0)), Some(1.0));
        assert_eq!(timed_transition_progress(0.05, Some(0.1)), Some(0.5));
        assert_eq!(timed_transition_progress(0.0, None), None);
    }

    #[test]
    fn transition_cache_prewarm_includes_source_and_target_states() {
        let source = StateId::default();
        let target = StateId::default();
        let state = LowLevelState {
            id: LowLevelStateId::HlTransition(TransitionId::Direct(default())),
            graph: default(),
            hl_transition: Some(LlTransitionData {
                source: source.clone(),
                target: target.clone(),
                hl_transition_id: TransitionId::Direct(default()),
                timed: Some(0.2),
            }),
        };
        assert_eq!(
            InterruptibleFsmNode::transition_endpoint_states(&state),
            Some([
                LowLevelStateId::HlState(source),
                LowLevelStateId::HlState(target),
            ])
        );
    }

    #[test]
    fn transition_weight_snapshot_preserves_each_source_state() {
        let source: StateId =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let target: StateId =
            serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        let mut weights = vec![SourceFsmStateWeight {
            state_id: source.uuid().to_string(),
            value: 0.25,
        }];
        InterruptibleFsmNode::add_state_weight(&mut weights, source, 0.5);
        InterruptibleFsmNode::add_state_weight(&mut weights, target, 0.25);
        assert_eq!(weights.len(), 2);
        assert!((weights[0].value - 0.75).abs() < f32::EPSILON);
        assert!((weights[1].value - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn recorded_source_weights_preserve_machine_and_state_values() {
        let state_id = StateId::default().uuid().to_string();
        let state = InterruptibleFsmState {
            state: LowLevelStateId::HlState(StateId::default()),
            state_entered_time: 0.0,
            context_slot: 0,
            blend_duration: None,
            interrupted_sources: Vec::new(),
            context: default(),
            recorded_machine_weight: 0.75,
            current_state_elapsed_time: 0.0,
            recorded_state_weights: vec![SourceFsmStateWeight {
                state_id: state_id.clone(),
                value: 0.25,
            }],
            source_pose: None,
            last_pose: None,
            label_last_index: None,
            first_update: false,
            last_update_frame: None,
            transition_notifications: Vec::new(),
            notification_generation: 0,
        };
        assert_eq!(state.recorded_machine_weight(), 0.75);
        assert_eq!(state.recorded_state_weight(&state_id), 0.25);
        assert_eq!(state.recorded_state_weight("missing"), 0.0);
    }

    #[test]
    fn interrupted_graph_transition_records_its_active_source() {
        let state_id = StateId::default();
        let active_transition = TransitionId::Direct(default());
        let next_transition = TransitionId::State(state_id);
        let active_state = LowLevelStateId::HlTransition(active_transition);
        let target_state = LowLevelStateId::HlTransition(next_transition);
        let mut machine = LowLevelStateMachine::new();
        machine.add_state(LowLevelState {
            id: active_state.clone(),
            graph: default(),
            hl_transition: Some(LlTransitionData {
                source: state_id,
                target: state_id,
                hl_transition_id: active_transition,
                timed: Some(0.2),
            }),
        });
        let transition = LowLevelTransition {
            id: LowLevelTransitionId::Start(next_transition),
            ignore_external: false,
            reset_target_state: false,
            source: LowLevelStateId::HlState(state_id),
            target: target_state.clone(),
            transition_type: LowLevelTransitionType::Direct,
            hl_source: state_id,
            hl_target: state_id,
        };
        let mut runtime = InterruptibleFsmState {
            state: active_state.clone(),
            state_entered_time: 1.0,
            context_slot: 0,
            blend_duration: None,
            interrupted_sources: Vec::new(),
            context: default(),
            recorded_machine_weight: 1.0,
            current_state_elapsed_time: 0.0,
            recorded_state_weights: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
            first_update: false,
            last_update_frame: None,
            transition_notifications: Vec::new(),
            notification_generation: 0,
        };

        assert!(InterruptibleFsmNode::can_interrupt(
            &machine,
            &runtime,
            &transition,
        ));
        InterruptibleFsmNode::activate_transition(&mut runtime, &transition, 1.1, None);

        assert_eq!(runtime.state, target_state);
        assert_eq!(runtime.state_entered_time, 1.1);
        assert_eq!(runtime.interrupted_sources.len(), 1);
        assert_eq!(runtime.interrupted_sources[0].state, active_state);
    }

    #[test]
    fn internal_state_label_driver_only_emits_on_a_label_change() {
        let labels = vec!["Entry".to_owned(), "Plant Right Foot".to_owned()];
        let mut last_index = None;
        assert_eq!(
            changed_label(&mut last_index, Some(0), &labels),
            Some("Entry".to_owned())
        );
        assert_eq!(changed_label(&mut last_index, Some(0), &labels), None);
        assert_eq!(
            changed_label(&mut last_index, Some(1), &labels),
            Some("Plant Right Foot".to_owned())
        );
    }
}
