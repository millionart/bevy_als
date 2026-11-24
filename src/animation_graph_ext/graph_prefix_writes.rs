//! Capture and replay frame writes without replacing per-character rig state.
//!
//! The pinned Animation Graph exposes its state and cache stores through Bevy reflection.
//! A different reflected layout uses ordinary graph evaluation.

use std::sync::Arc;

use bevy::{asset::AssetId, platform::collections::HashMap, reflect::structs::Struct};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, NodeId, TimeUpdate},
    context::{
        graph_context::{GraphState, QueryOutputTime},
        graph_context_arena::{GraphContextArena, GraphContextId, SubContextId},
        node_caches::{NodeCache, NodeCaches},
        node_state_box::NodeStateBox,
        node_states::{NodeState, NodeStates, StateKey},
    },
};

use super::source_instance::{SourceInstanceProperties, source_notify_instance_node};

type States = HashMap<NodeId, NodeState>;
type Caches = HashMap<NodeId, Arc<NodeCache>>;
type StateBoxes = HashMap<StateKey, Arc<NodeStateBox>>;
type Times = HashMap<StateKey, f32>;
pub(crate) type NotifyAliases = Vec<(u64, u64)>;

struct Layout {
    frame: u64,
    root: GraphContextId,
    contexts: Vec<(AssetId<AnimationGraph>, Option<SubContextId>)>,
}

impl Layout {
    fn read(arena: &GraphContextArena) -> Self {
        Self {
            frame: arena.frame_index(),
            root: arena.get_toplevel_id(),
            contexts: arena
                .iter_context_ids()
                .map(|id| {
                    (
                        arena.get_context(id).unwrap().get_graph_id(),
                        arena.get_parent_context(id).cloned(),
                    )
                })
                .collect(),
        }
    }

    fn matches(&self, arena: &GraphContextArena) -> bool {
        self.frame == arena.frame_index()
            && self.root == arena.get_toplevel_id()
            && self
                .contexts
                .iter()
                .map(|(graph, parent)| (*graph, parent.as_ref()))
                .eq(arena.iter_context_ids().map(|id| {
                    (
                        arena.get_context(id).unwrap().get_graph_id(),
                        arena.get_parent_context(id),
                    )
                }))
    }
}

struct Writes {
    id: GraphContextId,
    states: Vec<(NodeId, NodeState)>,
    caches: Vec<(NodeId, Arc<NodeCache>)>,
    time: Option<QueryOutputTime>,
}

pub(crate) struct GraphPrefixWrites {
    layout: Layout,
    contexts: Vec<Writes>,
    notify_owners: Vec<(GraphContextId, u64)>,
    writes_root_instance: bool,
}

fn state_map(value: &NodeStates) -> Option<&States> {
    value.field("states")?.try_downcast_ref()
}

fn state_map_mut(value: &mut NodeStates) -> Option<&mut States> {
    value.field_mut("states")?.try_downcast_mut()
}

fn cache_map(value: &NodeCaches) -> Option<&Caches> {
    value.field("caches")?.try_downcast_ref()
}

fn cache_map_mut(value: &mut NodeCaches) -> Option<&mut Caches> {
    value.field_mut("caches")?.try_downcast_mut()
}

struct StateStorage<'a> {
    last: &'a Option<Arc<NodeStateBox>>,
    upcoming: &'a StateBoxes,
    previous_time: f32,
    times: &'a Times,
}

impl<'a> StateStorage<'a> {
    fn read(state: &'a NodeState) -> Option<Self> {
        Some(Self {
            last: state.field("last_state")?.try_downcast_ref()?,
            upcoming: state.field("upcoming_state")?.try_downcast_ref()?,
            previous_time: state.get_last_time(),
            times: state.field("upcoming_time")?.try_downcast_ref()?,
        })
    }

    fn same(&self, other: &Self) -> bool {
        let same_last = match (&self.last, &other.last) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        same_last
            && self.previous_time.to_bits() == other.previous_time.to_bits()
            && self.upcoming.len() == other.upcoming.len()
            && self.upcoming.iter().all(|(key, state)| {
                other
                    .upcoming
                    .get(key)
                    .is_some_and(|previous| Arc::ptr_eq(state, previous))
            })
            && self.times.len() == other.times.len()
            && self.times.iter().all(|(key, time)| {
                other
                    .times
                    .get(key)
                    .is_some_and(|previous| time.to_bits() == previous.to_bits())
            })
    }
}

fn same_time(a: &TimeUpdate, b: &TimeUpdate) -> bool {
    match (a, b) {
        (TimeUpdate::Delta(a), TimeUpdate::Delta(b))
        | (TimeUpdate::Absolute(a), TimeUpdate::Absolute(b)) => a.to_bits() == b.to_bits(),
        (
            TimeUpdate::PercentOfEvent {
                percent: ap,
                event: ae,
                track: at,
            },
            TimeUpdate::PercentOfEvent {
                percent: bp,
                event: be,
                track: bt,
            },
        ) => ap.to_bits() == bp.to_bits() && ae == be && at == bt,
        _ => false,
    }
}

fn same_query_time(a: &QueryOutputTime, b: &QueryOutputTime) -> bool {
    match (a, b) {
        (QueryOutputTime::None, QueryOutputTime::None) => true,
        (QueryOutputTime::Forced(a), QueryOutputTime::Forced(b)) => same_time(a, b),
        (QueryOutputTime::ByKey(a), QueryOutputTime::ByKey(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, time)| b.get(key).is_some_and(|other| same_time(time, other)))
        }
        _ => false,
    }
}

impl GraphPrefixWrites {
    pub(crate) fn collect(arena: &GraphContextArena, before: &GraphContextArena) -> Option<Self> {
        let layout = Layout::read(before);
        if !layout.matches(arena) {
            return None;
        }
        let mut contexts = Vec::new();
        let mut notify_owners = Vec::new();
        let mut writes_root_instance = false;
        let notify_node = source_notify_instance_node();
        for id in arena.iter_context_ids() {
            let context = arena.get_context(id)?;
            let previous = before.get_context(id)?;
            let a = state_map(&context.node_states)?;
            let b = state_map(&previous.node_states)?;
            let c = cache_map(&context.node_caches)?;
            let d = cache_map(&previous.node_caches)?;
            if b.keys().any(|id| !a.contains_key(id)) || d.keys().any(|id| !c.contains_key(id)) {
                return None;
            }
            if let Ok(owner) = context
                .node_states
                .get::<SourceInstanceProperties>(notify_node, StateKey::Default)
            {
                notify_owners.push((id, owner.instance_id));
            }
            let mut states = Vec::new();
            for (&node, state) in a {
                if node == notify_node {
                    continue;
                }
                if let Some(previous) = b.get(&node)
                    && StateStorage::read(state)?.same(&StateStorage::read(previous)?)
                {
                    continue;
                }
                if id == layout.root
                    && node == NodeId::default()
                    && state
                        .get_state::<SourceInstanceProperties>(StateKey::Default)
                        .is_ok()
                {
                    writes_root_instance = true;
                }
                states.push((node, state.clone()));
            }
            let caches: Vec<_> = c
                .iter()
                .filter_map(|(&id, cache)| {
                    d.get(&id)
                        .is_none_or(|previous| !Arc::ptr_eq(cache, previous))
                        .then(|| (id, Arc::clone(cache)))
                })
                .collect();
            let time = (!same_query_time(&context.query_output_time, &previous.query_output_time))
                .then(|| context.query_output_time.clone());
            if !states.is_empty() || !caches.is_empty() || time.is_some() {
                contexts.push(Writes {
                    id,
                    states,
                    caches,
                    time,
                });
            }
        }
        Some(Self {
            layout,
            contexts,
            notify_owners,
            writes_root_instance,
        })
    }

    pub(crate) fn has_notify_owner(&self, instance: u64) -> bool {
        instance == 0 || self.notify_owners.iter().any(|(_, id)| *id == instance)
    }

    pub(crate) fn apply(&self, arena: &mut GraphContextArena) -> Option<NotifyAliases> {
        if !self.layout.matches(arena)
            || !self.contexts.iter().all(|writes| {
                arena
                    .get_context_mut(writes.id)
                    .is_some_and(|context: &mut GraphState| {
                        state_map_mut(&mut context.node_states).is_some()
                            && cache_map_mut(&mut context.node_caches).is_some()
                    })
            })
        {
            return None;
        }
        let root_instance = if self.writes_root_instance {
            Some(
                arena
                    .get_toplevel()
                    .node_states
                    .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
                    .ok()?
                    .instance_id,
            )
        } else {
            None
        };
        let mut aliases = Vec::with_capacity(self.notify_owners.len());
        let notify_node = source_notify_instance_node();
        for &(context, source) in &self.notify_owners {
            let local = arena
                .get_context(context)?
                .node_states
                .get::<SourceInstanceProperties>(notify_node, StateKey::Default)
                .ok()?
                .instance_id;
            aliases.push((source, local));
        }
        for writes in &self.contexts {
            let context = arena.get_context_mut(writes.id).unwrap();
            state_map_mut(&mut context.node_states)
                .unwrap()
                .extend(writes.states.iter().cloned());
            cache_map_mut(&mut context.node_caches)
                .unwrap()
                .extend(writes.caches.iter().cloned());
            if let Some(time) = &writes.time {
                context.query_output_time = time.clone();
            }
        }
        if let Some(instance_id) = root_instance {
            arena
                .get_toplevel_mut()
                .node_states
                .get_mut_or_insert_with(
                    NodeId::default(),
                    StateKey::Default,
                    SourceInstanceProperties::default,
                )
                .expect("the captured root has source instance state")
                .instance_id = instance_id;
        }
        Some(aliases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::Assets, asset::uuid::Uuid};

    #[test]
    fn replay_keeps_the_receiving_characters_rig_state() {
        let mut graphs = Assets::default();
        let graph = graphs.add(AnimationGraph::new());
        let mut source = GraphContextArena::new(graph.id());
        let mut receiver = GraphContextArena::new(graph.id());
        let animated: NodeId = Uuid::from_u128(1).into();
        let rig: NodeId = Uuid::from_u128(2).into();
        for (arena, rig_value) in [(&mut source, 1_u32), (&mut receiver, 2)] {
            *arena
                .get_toplevel_mut()
                .node_states
                .get_mut_or_insert_with(rig, StateKey::Default, || 0_u32)
                .unwrap() = rig_value;
        }
        let before = source.clone();
        let context = source.get_toplevel_mut();
        *context
            .node_states
            .get_mut_or_insert_with(animated, StateKey::Default, || 0_u32)
            .unwrap() = 7;
        context
            .node_states
            .set_time(animated, StateKey::Default, 0.25);
        context.node_caches.set_output_data(
            animated,
            StateKey::Default,
            "pose".into(),
            3.0_f32.into(),
        );
        let writes = GraphPrefixWrites::collect(&source, &before).unwrap();

        assert!(writes.apply(&mut receiver).is_some());
        let context = receiver.get_toplevel();
        assert_eq!(
            *context
                .node_states
                .get::<u32>(rig, StateKey::Default)
                .unwrap(),
            2
        );
        assert_eq!(
            *context
                .node_states
                .get::<u32>(animated, StateKey::Default)
                .unwrap(),
            7
        );
        assert_eq!(
            context.node_states.get_time(animated, StateKey::Default),
            0.25
        );
        assert_eq!(
            context
                .node_caches
                .get_output_data(animated, StateKey::Default, "pose".into())
                .unwrap()
                .as_f32()
                .unwrap(),
            3.0
        );

        receiver.next_frame();
        assert!(writes.apply(&mut receiver).is_none());
    }

    #[test]
    fn replay_keeps_notification_owners_local() {
        let mut graphs = Assets::default();
        let graph = graphs.add(AnimationGraph::new());
        let mut source = GraphContextArena::new(graph.id());
        let mut receiver = GraphContextArena::new(graph.id());
        let root = NodeId::default();
        let owner = source_notify_instance_node();
        for (arena, ids) in [(&mut source, [1, 2]), (&mut receiver, [3, 4])] {
            let states = &mut arena.get_toplevel_mut().node_states;
            for (node, instance_id) in [(root, ids[0]), (owner, ids[1])] {
                states
                    .get_mut_or_insert_with(
                        node,
                        StateKey::Default,
                        SourceInstanceProperties::default,
                    )
                    .unwrap()
                    .instance_id = instance_id;
            }
        }
        let before = source.clone();
        source
            .get_toplevel_mut()
            .node_states
            .get_mut_or_insert_with(root, StateKey::Default, SourceInstanceProperties::default)
            .unwrap()
            .values
            .insert("test".into(), 2.0_f32.into());

        let writes = GraphPrefixWrites::collect(&source, &before).unwrap();
        assert_eq!(writes.apply(&mut receiver), Some(vec![(2, 4)]));
        let states = &receiver.get_toplevel().node_states;
        let state = states
            .get::<SourceInstanceProperties>(root, StateKey::Default)
            .unwrap();
        assert_eq!(state.instance_id, 3);
        assert_eq!(state.values["test"].as_f32().unwrap(), 2.0);
        assert_eq!(
            states
                .get::<SourceInstanceProperties>(owner, StateKey::Default)
                .unwrap()
                .instance_id,
            4
        );
    }
}
