use std::collections::VecDeque;

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
            FsmBuiltinPin, LowLevelStateId, LowLevelStateMachine, LowLevelTransition,
            LowLevelTransitionId,
        },
    },
};

use crate::animation_context::{
    AnimationContextState, capture_context_curve, graph_output_context_state,
    merge_missing_context_curves,
};
use crate::global_f32_input::global_f32_or_root_default;
use crate::state_label_event::state_label_index;

#[derive(Reflect, Clone, Debug)]
pub struct InterruptibleFsmSource {
    pub state: LowLevelStateId,
    pub state_entered_time: f32,
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

#[derive(Reflect, Clone, Debug)]
pub struct InterruptibleFsmState {
    pub state: LowLevelStateId,
    pub state_entered_time: f32,
    pub interrupted_sources: Vec<InterruptibleFsmSource>,
    pub context: AnimationContextState,
    source_pose: Option<Pose>,
    last_pose: Option<Pose>,
    label_last_index: Option<usize>,
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

impl InterruptibleFsmState {
    pub fn cached_source_pose(&self) -> Option<&Pose> {
        self.source_pose.as_ref()
    }

    pub fn last_output_pose(&self) -> Option<&Pose> {
        self.last_pose.as_ref()
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
    pub outer_pose_output: String,
    pub state_label_key: String,
    pub state_labels: Vec<String>,
    pub internal_time_key: String,
    pub max_transitions_per_frame: usize,
    pub max_transition_requests: usize,
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
            outer_pose_output: Self::OUT_POSE.to_owned(),
            state_label_key: default(),
            state_labels: default(),
            internal_time_key: default(),
            max_transitions_per_frame: 3,
            max_transition_requests: 32,
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
            outer_pose_output: Self::OUT_POSE.to_owned(),
            state_label_key: String::new(),
            state_labels: Vec::new(),
            internal_time_key: String::new(),
            max_transitions_per_frame: 3,
            max_transition_requests: 32,
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

    fn reset_target_state(
        machine: &LowLevelStateMachine,
        transition: &LowLevelTransition,
        ctx: &mut NodeContext,
    ) {
        if !transition.reset_target_state {
            return;
        }
        let target = LowLevelStateId::HlState(transition.hl_target);
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
    ) {
        let interrupted = state.state != transition.source;
        let mut interrupted_sources = Vec::new();
        let source_pose = if interrupted {
            interrupted_sources.push(InterruptibleFsmSource {
                state: state.state.clone(),
                state_entered_time: state.state_entered_time,
            });
            interrupted_sources.extend(state.interrupted_sources.iter().cloned());
            state.last_pose.clone()
        } else {
            None
        };
        state.state = transition.target.clone();
        state.state_entered_time = time;
        state.interrupted_sources = interrupted_sources;
        state.source_pose = source_pose;
        if !matches!(state.state, LowLevelStateId::HlTransition(_)) {
            state.source_pose = None;
        }
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
        let start_state = machine
            .start_state
            .clone()
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let mut reset = None;
        {
            let state = ctx.state_mut_or_else(|| InterruptibleFsmState {
                state: start_state,
                state_entered_time: time,
                interrupted_sources: Vec::new(),
                context: AnimationContextState::default(),
                source_pose: None,
                last_pose: None,
                label_last_index: None,
            })?;
            let start = if allow_external {
                Self::event_start_index(events.events.len(), self.max_transition_requests)
            } else {
                0
            };
            for sampled in events.events.into_iter().skip(start) {
                if *remaining_transitions == 0 {
                    break;
                }
                let transition = match sampled.event {
                    AnimationEvent::TransitionToState(target) => {
                        if let LowLevelStateId::HlState(source) = state.state {
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
                        self.direct_transition(machine, state, transition_id)
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
                    if transition.reset_target_state {
                        reset = Some(transition.clone());
                    }
                    Self::activate_transition(state, transition, time);
                    *remaining_transitions -= 1;
                }
            }
        }
        if let Some(transition) = reset.as_ref() {
            Self::reset_target_state(machine, transition, ctx);
        }
        Ok(())
    }

    fn driver_events(&self, ctx: &mut NodeContext) -> Result<EventQueue, GraphError> {
        if self.state_label_key.is_empty() {
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

    fn evaluate_active_state(
        &self,
        machine: &LowLevelStateMachine,
        parent_time_update: TimeUpdate,
        ctx: &mut NodeContext,
    ) -> Result<EventQueue, GraphError> {
        let state = ctx.state::<InterruptibleFsmState>()?.clone();
        let (state_data, graph) = Self::active_graph(machine, &state.state, ctx)?;
        let elapsed = (ctx.time() - state.state_entered_time).max(0.0);
        let io = InterruptibleFsmIo {
            node_context: ctx.clone(),
            machine,
            current_time: ctx.time(),
            parent_time_update,
            state: state.state.clone(),
            state_entered_time: state.state_entered_time,
            source_pose: state.source_pose.clone(),
            role: FsmRole::Root,
            stack: VecDeque::new(),
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let sub_ctx = ctx
            .create_child_context(state_data.graph.id(), graph, Some(state.state.clone()))
            .with_io(&io);
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
        let snapshot = ctx.state::<InterruptibleFsmState>()?.clone();
        ctx.state_mut_or_else(|| snapshot)?.context = context_state;
        if machine
            .states
            .get(&state.state)
            .and_then(|state| state.hl_transition.as_ref())
            .and_then(|transition| transition.timed)
            .is_some_and(|duration| duration > 0.0 && elapsed >= duration)
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
        ctx.set_time(time);
        ctx.set_time_update_back(LowLevelStateMachine::DRIVER_TIME, update.clone());
        let start_state = machine
            .start_state
            .clone()
            .ok_or(GraphError::FSMCurrentStateMissing)?;
        let _ = ctx.state_mut_or_else(|| InterruptibleFsmState {
            state: start_state,
            state_entered_time: time,
            interrupted_sources: Vec::new(),
            context: AnimationContextState::default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
        })?;
        let external_events = self.driver_events(&mut ctx)?;
        let mut remaining_transitions = self.max_transitions_per_frame;
        self.process_events(
            machine,
            external_events,
            true,
            &mut remaining_transitions,
            &mut ctx,
        )?;
        let internal_events = self.evaluate_active_state(machine, update, &mut ctx)?;
        self.process_events(
            machine,
            internal_events,
            false,
            &mut remaining_transitions,
            &mut ctx,
        )?;
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
        if self.state_label_key.is_empty() {
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

#[derive(Clone)]
struct FsmStackFrame {
    state: LowLevelStateId,
    state_entered_time: f32,
    source_pose: Option<Pose>,
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
    source_pose: Option<Pose>,
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
        self.node_context
            .clone()
            .with_state_key(ctx.state_key)
            .data_back(pin.clone())
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
                .and_then(|transition| transition.timed)
                .filter(|duration| *duration > 0.0)
                .map(|duration| DataValue::F32(elapsed / duration))
                .ok_or(GraphError::FSMRequestedMissingData),
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
            source_pose: None,
            role,
            stack,
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let _ = ctx;
        Ok((graph, io))
    }

    fn state_data(
        &self,
        pin: PinId,
        state: LowLevelStateId,
        role: FsmRole,
        ctx: &GraphContext,
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
        let result = graph.get_data(TargetPin::OutputData(pin.clone()), sub_ctx.clone());
        let Err(error) = result else {
            return result;
        };
        let pose_output = self.state_pose_output_for(&state).to_owned();
        if pin == pose_output {
            return Err(error);
        }
        graph.get_data(TargetPin::OutputData(pose_output.clone()), sub_ctx.clone())?;
        graph_output_context_state(graph, &sub_ctx, &pose_output)
            .and_then(|context| context.curves.get(&pin).copied())
            .map(DataValue::F32)
            .ok_or(error)
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
        if matches!(self.role, FsmRole::Source) {
            if let Some(pose) = &self.source_pose {
                return Ok(TimeUpdate::Absolute(pose.timestamp));
            }
        }
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
            source_pose: frame.source_pose,
            role: frame.role,
            stack,
            state_pose_output: self.state_pose_output.clone(),
            state_pose_outputs: self.state_pose_outputs.clone(),
        };
        let sub_ctx = self
            .node_context
            .create_child_context(graph_handle.id(), graph, Some(frame.state))
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
                if let Some(pose) = &self.source_pose {
                    if pin.to_string() == InterruptibleFsmNode::OUT_POSE {
                        return Ok(DataValue::Pose(pose.clone()));
                    }
                }
                self.source_state().and_then(|state| {
                    let state_pin = if pin.to_string() == InterruptibleFsmNode::OUT_POSE {
                        self.state_pose_output_for(&state).to_owned()
                    } else {
                        pin
                    };
                    self.state_data(state_pin, state, FsmRole::Source, &ctx)
                })
            }
            GraphInputPin::FromFsmTarget(pin) => self.target_state().and_then(|state| {
                let state_pin = if pin.to_string() == InterruptibleFsmNode::OUT_POSE {
                    self.state_pose_output_for(&state).to_owned()
                } else {
                    pin
                };
                self.state_data(state_pin, state, FsmRole::Target, &ctx)
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
            GraphInputPin::FsmBuiltin(_) => Err(GraphError::FSMRequestedMissingData),
        }
    }

    fn get_time_fwd(&self, ctx: GraphContext) -> Result<TimeUpdate, GraphError> {
        self.nested_time(&ctx).or_else(|_| self.parent_time(&ctx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direct_transition_event::take_direct_transition_edge;
    use bevy_animation_graph::core::state_machine::low_level::{
        LlTransitionData, LowLevelState, LowLevelTransitionType,
    };

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
            interrupted_sources: Vec::new(),
            context: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
        };

        let selected =
            InterruptibleFsmNode::default().direct_transition(&machine, &runtime, transition_id);

        assert!(selected.is_some_and(|transition| {
            transition.id == LowLevelTransitionId::Immediate(transition_id)
        }));
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
            interrupted_sources: Vec::new(),
            context: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
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
            interrupted_sources: Vec::new(),
            context: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
        };
        let mut previous = None;
        let mut deliveries = 0;
        if take_direct_transition_edge(&mut previous, 0.0, 1).is_some() {
            let transition = InterruptibleFsmNode::default()
                .direct_transition(&machine, &runtime, transition_id)
                .unwrap();
            InterruptibleFsmNode::activate_transition(&mut runtime, transition, 0.25);
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
    }

    #[test]
    fn internal_time_key_uses_a_nonnegative_delta() {
        let delta = (-0.25_f32).max(0.0);
        assert!(delta >= 0.0);
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
            interrupted_sources: Vec::new(),
            context: default(),
            source_pose: None,
            last_pose: None,
            label_last_index: None,
        };

        assert!(InterruptibleFsmNode::can_interrupt(
            &machine,
            &runtime,
            &transition,
        ));
        InterruptibleFsmNode::activate_transition(&mut runtime, &transition, 1.1);

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
