use bevy::platform::collections::{HashMap, HashSet};
use bevy_animation_graph::core::edge_data::events::{AnimationEvent, EventQueue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedEventKind {
    Point,
    State,
    Marker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateIntervalBoundary {
    StartInclusiveEndExclusive,
    StartExclusiveEndInclusive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimedEvent {
    pub id: String,
    pub kind: TimedEventKind,
    pub start_time: f32,
    pub end_time: f32,
    pub weight_threshold: f32,
    pub track: Option<String>,
    pub state_boundary: StateIntervalBoundary,
    pub trigger_on_follower: bool,
    pub trigger_on_dedicated_server: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventClock {
    pub previous_time: f32,
    pub current_time: f32,
    pub duration: f32,
    pub direction: PlaybackDirection,
    pub wrapped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDispatchContext {
    pub is_leader: bool,
    pub is_dedicated_server: bool,
}

impl Default for EventDispatchContext {
    fn default() -> Self {
        Self {
            is_leader: true,
            is_dedicated_server: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Begin,
    Active,
    End,
    Point,
    Marker,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventLifecycleSignal {
    pub id: String,
    pub phase: EventPhase,
    pub weight: f32,
    pub percentage: f32,
    pub track: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveState {
    weight: f32,
    percentage: f32,
    track: Option<String>,
}

#[derive(Debug, Default)]
pub struct EventLifecycle {
    active_states: HashMap<String, ActiveState>,
}

impl EventLifecycle {
    pub fn active_count(&self) -> usize {
        self.active_states.len()
    }

    pub fn reset(&mut self) -> Vec<EventLifecycleSignal> {
        self.end_states(|_| true)
    }

    pub(crate) fn end_states(
        &mut self,
        should_end: impl Fn(&str) -> bool,
    ) -> Vec<EventLifecycleSignal> {
        let mut ids = self
            .active_states
            .keys()
            .filter(|id| should_end(id))
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        let mut output = Vec::with_capacity(ids.len());
        for id in ids {
            let state = self
                .active_states
                .remove(&id)
                .expect("active state must exist");
            output.push(EventLifecycleSignal {
                id,
                phase: EventPhase::End,
                weight: state.weight,
                percentage: state.percentage,
                track: state.track,
            });
        }
        output
    }

    pub fn update(
        &mut self,
        definitions: &[TimedEvent],
        queue: &EventQueue,
        clock: Option<EventClock>,
        point_weight: f32,
    ) -> Vec<EventLifecycleSignal> {
        self.update_with_context(
            definitions,
            queue,
            clock,
            point_weight,
            EventDispatchContext::default(),
        )
    }

    pub fn update_with_context(
        &mut self,
        definitions: &[TimedEvent],
        queue: &EventQueue,
        clock: Option<EventClock>,
        point_weight: f32,
        dispatch: EventDispatchContext,
    ) -> Vec<EventLifecycleSignal> {
        let definitions = definitions
            .iter()
            .map(|definition| (definition.id.as_str(), definition))
            .collect::<HashMap<_, _>>();
        let mut current = HashMap::<String, ActiveState>::default();
        for sample in &queue.events {
            let AnimationEvent::StringId(id) = &sample.event else {
                continue;
            };
            let Some(definition) = definitions.get(id.as_str()) else {
                continue;
            };
            if definition.kind != TimedEventKind::State
                || definition.state_boundary != StateIntervalBoundary::StartInclusiveEndExclusive
                || sample.weight < definition.weight_threshold
                || !passes_dispatch(definition, dispatch)
            {
                continue;
            }
            let entry = ActiveState {
                weight: sample.weight,
                percentage: sample.percentage,
                track: sample.track.clone(),
            };
            match current.get(id) {
                Some(existing) if existing.weight >= entry.weight => {}
                _ => {
                    current.insert(id.clone(), entry);
                }
            }
        }

        if let Some(clock) = clock {
            for definition in definitions.values().copied() {
                if definition.kind != TimedEventKind::State
                    || definition.state_boundary
                        != StateIntervalBoundary::StartExclusiveEndInclusive
                    || point_weight < definition.weight_threshold
                    || !passes_dispatch(definition, dispatch)
                    || !state_active_at(definition, clock.current_time)
                {
                    continue;
                }
                current.insert(
                    definition.id.clone(),
                    ActiveState {
                        weight: point_weight,
                        percentage: state_percentage(definition, clock.current_time),
                        track: definition.track.clone(),
                    },
                );
            }
        }

        let previously_active = self.active_states.keys().cloned().collect::<HashSet<_>>();
        let mut output = self.end_states(|id| !current.contains_key(id));

        let mut current_ids = current.keys().cloned().collect::<Vec<_>>();
        current_ids.sort();
        let current_state_ids = current_ids.iter().cloned().collect::<HashSet<_>>();
        for id in &current_ids {
            let state = current.get(id).expect("current state must exist");
            if !self.active_states.contains_key(id) {
                output.push(EventLifecycleSignal {
                    id: id.clone(),
                    phase: EventPhase::Begin,
                    weight: state.weight,
                    percentage: state.percentage,
                    track: state.track.clone(),
                });
            }
        }
        for id in current_ids {
            let state = current.remove(&id).expect("current state must exist");
            output.push(EventLifecycleSignal {
                id: id.clone(),
                phase: EventPhase::Active,
                weight: state.weight,
                percentage: state.percentage,
                track: state.track.clone(),
            });
            self.active_states.insert(id, state);
        }

        let Some(clock) = clock else {
            return output;
        };
        let mut skipped_states = definitions
            .values()
            .copied()
            .filter(|definition| {
                definition.kind == TimedEventKind::State
                    && point_weight >= definition.weight_threshold
                    && passes_dispatch(definition, dispatch)
                    && !current_state_ids.contains(&definition.id)
                    && !previously_active.contains(&definition.id)
                    && crossed_time(definition.start_time, clock)
                    && crossed_time(definition.end_time, clock)
            })
            .collect::<Vec<_>>();
        skipped_states.sort_by(|first, second| point_order(first, second, clock.direction));
        for definition in skipped_states {
            output.push(EventLifecycleSignal {
                id: definition.id.clone(),
                phase: EventPhase::Begin,
                weight: point_weight,
                percentage: 0.0,
                track: definition.track.clone(),
            });
            output.push(EventLifecycleSignal {
                id: definition.id.clone(),
                phase: EventPhase::End,
                weight: point_weight,
                percentage: 1.0,
                track: definition.track.clone(),
            });
        }
        let mut crossings = definitions
            .values()
            .copied()
            .filter(|definition| {
                definition.kind != TimedEventKind::State
                    && point_weight >= definition.weight_threshold
                    && passes_dispatch(definition, dispatch)
                    && crossed_time(definition.start_time, clock)
            })
            .collect::<Vec<_>>();
        crossings.sort_by(|first, second| point_order(first, second, clock.direction));
        for definition in crossings {
            output.push(EventLifecycleSignal {
                id: definition.id.clone(),
                phase: match definition.kind {
                    TimedEventKind::Point => EventPhase::Point,
                    TimedEventKind::Marker => EventPhase::Marker,
                    TimedEventKind::State => unreachable!(),
                },
                weight: point_weight,
                percentage: 1.0,
                track: definition.track.clone(),
            });
        }
        output
    }
}

pub fn crossed_time(time: f32, clock: EventClock) -> bool {
    match (clock.direction, clock.wrapped) {
        (PlaybackDirection::Forward, false) => {
            clock.previous_time < time && time <= clock.current_time
        }
        (PlaybackDirection::Forward, true) => {
            time > clock.previous_time || time <= clock.current_time
        }
        (PlaybackDirection::Backward, false) => {
            clock.current_time <= time && time < clock.previous_time
        }
        (PlaybackDirection::Backward, true) => {
            time < clock.previous_time || time >= clock.current_time
        }
    }
}

fn point_order(
    first: &TimedEvent,
    second: &TimedEvent,
    direction: PlaybackDirection,
) -> std::cmp::Ordering {
    let order = first.start_time.total_cmp(&second.start_time);
    match direction {
        PlaybackDirection::Forward => order.then_with(|| first.id.cmp(&second.id)),
        PlaybackDirection::Backward => order.reverse().then_with(|| first.id.cmp(&second.id)),
    }
}

fn state_active_at(definition: &TimedEvent, time: f32) -> bool {
    match definition.state_boundary {
        StateIntervalBoundary::StartInclusiveEndExclusive => {
            definition.start_time <= time && time < definition.end_time
        }
        StateIntervalBoundary::StartExclusiveEndInclusive => {
            definition.start_time < time && time <= definition.end_time
        }
    }
}

fn state_percentage(definition: &TimedEvent, time: f32) -> f32 {
    let duration = definition.end_time - definition.start_time;
    if duration <= f32::EPSILON {
        return 1.0;
    }
    ((time - definition.start_time) / duration).clamp(0.0, 1.0)
}

fn passes_dispatch(definition: &TimedEvent, context: EventDispatchContext) -> bool {
    (context.is_leader || definition.trigger_on_follower)
        && (!context.is_dedicated_server || definition.trigger_on_dedicated_server)
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::edge_data::events::{AnimationEvent, SampledEvent};

    use super::*;

    fn definition(
        id: &str,
        kind: TimedEventKind,
        start: f32,
        end: f32,
        threshold: f32,
    ) -> TimedEvent {
        TimedEvent {
            id: id.into(),
            kind,
            start_time: start,
            end_time: end,
            weight_threshold: threshold,
            track: Some("track".into()),
            state_boundary: StateIntervalBoundary::StartInclusiveEndExclusive,
            trigger_on_follower: false,
            trigger_on_dedicated_server: true,
        }
    }

    fn state_queue(id: &str, weight: f32, percentage: f32) -> EventQueue {
        EventQueue::with_events([SampledEvent {
            event: AnimationEvent::StringId(id.into()),
            weight,
            percentage,
            track: Some("track".into()),
        }])
    }

    fn forward(previous_time: f32, current_time: f32, wrapped: bool) -> EventClock {
        EventClock {
            previous_time,
            current_time,
            duration: 1.0,
            direction: PlaybackDirection::Forward,
            wrapped,
        }
    }

    #[test]
    fn state_emits_begin_active_and_end_once() {
        let state = definition("state", TimedEventKind::State, 0.0, 1.0, 0.5);
        let mut lifecycle = EventLifecycle::default();
        let begin = lifecycle.update(
            &[state.clone()],
            &state_queue("state", 0.75, 0.25),
            None,
            0.0,
        );
        assert_eq!(
            begin.iter().map(|signal| signal.phase).collect::<Vec<_>>(),
            vec![EventPhase::Begin, EventPhase::Active]
        );
        let active = lifecycle.update(
            &[state.clone()],
            &state_queue("state", 0.75, 0.5),
            None,
            0.0,
        );
        assert_eq!(
            active.iter().map(|signal| signal.phase).collect::<Vec<_>>(),
            vec![EventPhase::Active]
        );
        let end = lifecycle.update(&[state], &EventQueue::default(), None, 0.0);
        assert_eq!(
            end.iter().map(|signal| signal.phase).collect::<Vec<_>>(),
            vec![EventPhase::End]
        );
    }

    #[test]
    fn state_respects_weight_threshold() {
        let state = definition("state", TimedEventKind::State, 0.0, 1.0, 0.5);
        let mut lifecycle = EventLifecycle::default();
        let signals = lifecycle.update(&[state], &state_queue("state", 0.49, 0.5), None, 0.0);
        assert!(signals.is_empty());
    }

    #[test]
    fn point_and_marker_fire_only_when_crossed() {
        let point = definition("point", TimedEventKind::Point, 0.25, 0.25, 0.5);
        let marker = definition("marker", TimedEventKind::Marker, 0.75, 0.75, 0.5);
        let mut lifecycle = EventLifecycle::default();
        let none = lifecycle.update(
            &[point.clone(), marker.clone()],
            &EventQueue::default(),
            Some(forward(0.0, 0.2, false)),
            1.0,
        );
        assert!(none.is_empty());
        let point_signal = lifecycle.update(
            &[point.clone(), marker.clone()],
            &EventQueue::default(),
            Some(forward(0.2, 0.3, false)),
            1.0,
        );
        assert_eq!(point_signal[0].phase, EventPhase::Point);
        let marker_signal = lifecycle.update(
            &[point, marker],
            &EventQueue::default(),
            Some(forward(0.7, 0.8, false)),
            1.0,
        );
        assert_eq!(marker_signal[0].phase, EventPhase::Marker);
    }

    #[test]
    fn forward_and_backward_wraps_cross_each_point_once() {
        let point = definition("point", TimedEventKind::Point, 0.1, 0.1, 0.0);
        let mut lifecycle = EventLifecycle::default();
        let forward_wrap = lifecycle.update(
            &[point.clone()],
            &EventQueue::default(),
            Some(forward(0.9, 0.2, true)),
            1.0,
        );
        assert_eq!(forward_wrap.len(), 1);
        let backward = EventClock {
            previous_time: 0.2,
            current_time: 0.9,
            duration: 1.0,
            direction: PlaybackDirection::Backward,
            wrapped: true,
        };
        let backward_wrap = lifecycle.update(&[point], &EventQueue::default(), Some(backward), 1.0);
        assert_eq!(backward_wrap.len(), 1);
    }

    #[test]
    fn reset_ends_each_active_state_without_affecting_another_lifecycle() {
        let state = definition("state", TimedEventKind::State, 0.0, 1.0, 0.0);
        let mut first = EventLifecycle::default();
        let mut second = EventLifecycle::default();
        first.update(&[state.clone()], &state_queue("state", 1.0, 0.5), None, 0.0);
        second.update(&[state], &EventQueue::default(), None, 0.0);
        assert_eq!(first.reset().len(), 1);
        assert_eq!(second.active_count(), 0);
    }

    #[test]
    fn skipped_state_interval_emits_begin_then_end() {
        let state = definition("state", TimedEventKind::State, 0.25, 0.5, 0.5);
        let mut lifecycle = EventLifecycle::default();
        let signals = lifecycle.update(
            &[state],
            &EventQueue::default(),
            Some(forward(0.1, 0.9, false)),
            0.75,
        );
        assert_eq!(
            signals
                .iter()
                .map(|signal| signal.phase)
                .collect::<Vec<_>>(),
            vec![EventPhase::Begin, EventPhase::End]
        );
    }

    #[test]
    fn native_branching_state_uses_exclusive_start_and_inclusive_end() {
        let mut state = definition("state", TimedEventKind::State, 0.25, 0.5, 0.0);
        state.state_boundary = StateIntervalBoundary::StartExclusiveEndInclusive;
        let mut lifecycle = EventLifecycle::default();
        let at_start = lifecycle.update(
            &[state.clone()],
            &EventQueue::default(),
            Some(forward(0.2, 0.25, false)),
            1.0,
        );
        assert!(at_start.is_empty());
        let during = lifecycle.update(
            &[state.clone()],
            &EventQueue::default(),
            Some(forward(0.25, 0.3, false)),
            1.0,
        );
        assert_eq!(during[0].phase, EventPhase::Begin);
        let at_end = lifecycle.update(
            &[state.clone()],
            &EventQueue::default(),
            Some(forward(0.3, 0.5, false)),
            1.0,
        );
        assert_eq!(at_end[0].phase, EventPhase::Active);
        let after_end = lifecycle.update(
            &[state],
            &EventQueue::default(),
            Some(forward(0.5, 0.6, false)),
            1.0,
        );
        assert_eq!(after_end[0].phase, EventPhase::End);
    }

    #[test]
    fn follower_and_dedicated_server_policy_filter_events() {
        let point = definition("point", TimedEventKind::Point, 0.25, 0.25, 0.0);
        let clock = Some(forward(0.2, 0.3, false));
        let mut lifecycle = EventLifecycle::default();
        let follower = lifecycle.update_with_context(
            &[point.clone()],
            &EventQueue::default(),
            clock,
            1.0,
            EventDispatchContext {
                is_leader: false,
                is_dedicated_server: false,
            },
        );
        assert!(follower.is_empty());
        let mut dedicated_point = point;
        dedicated_point.trigger_on_dedicated_server = false;
        let dedicated = lifecycle.update_with_context(
            &[dedicated_point],
            &EventQueue::default(),
            clock,
            1.0,
            EventDispatchContext {
                is_leader: true,
                is_dedicated_server: true,
            },
        );
        assert!(dedicated.is_empty());
    }
}
