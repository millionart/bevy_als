use std::{
    cell::RefCell,
    sync::{Arc, LazyLock, Mutex, OnceLock},
};

use bevy::{
    asset::{AssetId, AssetPath},
    platform::collections::HashMap,
    prelude::{Entity, Resource},
    reflect::structs::Struct,
};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, NodeId, TimeUpdate},
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    context::{
        graph_context_arena::{GraphContextArena, GraphContextId, SubContextId},
        io_env::IoOverrides,
        new_context::NodeContext,
        node_states::StateKey,
    },
    edge_data::{DataValue, events::EventQueue},
    errors::GraphError,
    pose::Pose,
    skeleton::Skeleton,
};

use crate::{
    AlsAnimationLod,
    als::transitions::TransitionSequence,
    extensions::{
        animation_context::pose_input_context,
        graph_prefix_writes::{GraphPrefixWrites, NotifyAliases},
        sharing_asset_revisions::AssetRevision,
        source_gameplay_tag_input::{SourceAnimationInputs, active_pre_rig_inputs},
        source_instance::source_input_data,
        sync_group_phase::{SourceSyncPlayerTick, source_sync_preview},
    },
};

/// Shares the compatible stock ALS graph stages before each character's local Control Rig solve.
/// The setting takes effect as compatible animation players start their graphs.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct AlsPreRigSharingSettings {
    pub enabled: bool,
}

#[derive(Clone)]
pub(crate) struct ActorScope {
    entity: Entity,
    skeleton: AssetId<Skeleton>,
    omitted_bones: usize,
    curve_filter: usize,
    overrides: Arc<HashMap<GraphInputPin, DataValue>>,
}

impl ActorScope {
    pub(crate) fn for_player(
        entity: Entity,
        player: &AnimationGraphPlayer,
        lod: Option<&AlsAnimationLod>,
    ) -> Option<Self> {
        static ROOT_GRAPH: LazyLock<AssetPath<'static>> =
            LazyLock::new(|| crate::als_asset_path("ALS/Character/AB_Als.runtime.animgraph.ron"));
        let AnimationSource::Graph(graph) = player.get_animation_source() else {
            return None;
        };
        if graph.path() != Some(&*ROOT_GRAPH)
            || player.is_paused()
            || lod.is_some_and(AlsAnimationLod::requires_continuous_pose)
        {
            return None;
        }
        let events = player
            .field("queued_events")?
            .try_downcast_ref::<EventQueue>()?;
        let inputs = player
            .field("io_overrides")?
            .try_downcast_ref::<IoOverrides>()?;
        if !events.events.is_empty() || !inputs.duration.is_empty() || inputs.time.is_some() {
            return None;
        }
        let overrides = inputs
            .data
            .iter()
            .filter(|(pin, _)| {
                // The player replaces this pin with queued_events at the start of evaluation.
                !matches!(pin, GraphInputPin::Passthrough(name) if name == AnimationGraphPlayer::USER_EVENTS)
            })
            .map(|(pin, value)| (pin.clone(), value.clone()))
            .collect();
        let skeleton = player.skeleton().id();
        Some(Self {
            entity,
            skeleton,
            omitted_bones: lod
                .and_then(|lod| lod.omitted_bone_plan(skeleton))
                .map_or(0, |bones| Arc::as_ptr(bones) as usize),
            curve_filter: lod
                .and_then(|lod| lod.curve_filter.as_ref())
                .map_or(0, |filter| Arc::as_ptr(filter) as usize),
            overrides: Arc::new(overrides),
        })
    }

    fn matches_policy(&self, other: &Self) -> bool {
        self.skeleton == other.skeleton
            && self.omitted_bones == other.omitted_bones
            && self.curve_filter == other.curve_filter
            && self.overrides.len() == other.overrides.len()
            && self.overrides.iter().all(|(pin, value)| {
                other
                    .overrides
                    .get(pin)
                    .is_some_and(|other| same_value(value, other))
            })
    }
}

thread_local! {
    static ACTOR_SCOPE: RefCell<Option<ActorScope>> = const { RefCell::new(None) };
}

pub(crate) fn with_actor_scope<T>(scope: Option<ActorScope>, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<ActorScope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTOR_SCOPE.with(|scope| scope.replace(self.0.take()));
        }
    }
    let _restore = Restore(ACTOR_SCOPE.with(|current| current.replace(scope)));
    run()
}

fn actor_scope(ctx: &NodeContext) -> Option<ActorScope> {
    ACTOR_SCOPE.with_borrow(|scope| {
        scope
            .as_ref()
            .filter(|actor| {
                actor.entity == ctx.graph_context.root_entity
                    && ctx.graph_context.state_key == StateKey::Default
            })
            .cloned()
    })
}

type Globals = HashMap<String, DataValue>;
type Actor = (Entity, GraphContextId, NodeId);
type Layout = Vec<(AssetId<AnimationGraph>, Option<SubContextId>)>;
type FootTransition = Option<TransitionSequence>;

fn context_layout(
    arena: &GraphContextArena,
) -> impl Iterator<Item = (AssetId<AnimationGraph>, Option<&SubContextId>)> {
    arena.iter_context_ids().map(|id| {
        (
            arena.get_context(id).unwrap().get_graph_id(),
            arena.get_parent_context(id),
        )
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    graph: AssetId<AnimationGraph>,
    context: GraphContextId,
    node: NodeId,
    frame: u64,
    previous: u64,
    delta: bool,
    time: u32,
}

impl Key {
    fn new(ctx: &NodeContext, previous: u64, update: &TimeUpdate) -> Option<Self> {
        let (delta, time) = match update {
            TimeUpdate::Delta(time) => (true, time.to_bits()),
            TimeUpdate::Absolute(time) => (false, time.to_bits()),
            TimeUpdate::PercentOfEvent { .. } => return None,
        };
        Some(Self {
            graph: ctx.graph_context.context().get_graph_id(),
            context: ctx.graph_context.context_id,
            node: ctx.node_id,
            frame: ctx.graph_context.frame_index(),
            previous,
            delta,
            time,
        })
    }
}

struct Inputs {
    policy: ActorScope,
    source: SourceAnimationInputs,
    globals: Globals,
    foot_transition: Option<FootTransition>,
}

impl Inputs {
    fn matches(
        &self,
        policy: &ActorScope,
        source: &SourceAnimationInputs,
        globals: &Globals,
        foot: Option<FootTransition>,
    ) -> bool {
        self.policy.matches_policy(policy)
            && self.source.pre_rig_compatible(source)
            && self.foot_transition == foot
            && same_globals(&self.globals, globals, foot.is_some())
    }
}

struct CapturedPrefix {
    writes: GraphPrefixWrites,
    pose: Pose,
}

struct PrefixEntry {
    key: Key,
    lineage: u64,
    inputs: Inputs,
    captured: OnceLock<Option<CapturedPrefix>>,
}

struct PreviewEntry {
    key: Key,
    layout: Layout,
    inputs: Inputs,
    ticks: OnceLock<Option<Vec<SourceSyncPlayerTick>>>,
}

#[derive(Clone, Copy)]
struct History {
    frame: u64,
    epoch: u64,
    lineage: u64,
}

#[derive(Default)]
struct Frame {
    pending_asset_events: bool,
    epoch: u64,
    next_lineage: u64,
    reused_players: usize,
    histories: HashMap<Actor, History>,
    cohort_sizes: HashMap<u64, usize>,
    prefixes: Vec<Arc<PrefixEntry>>,
    previews: Vec<Arc<PreviewEntry>>,
}

#[derive(Default)]
pub(crate) struct PreRigSharing(Mutex<Frame>);

impl PreRigSharing {
    pub(crate) fn begin_frame(&self, enabled: bool, revision: AssetRevision) -> usize {
        let mut frame = self.0.lock().unwrap();
        let reused = std::mem::take(&mut frame.reused_players);
        frame.prefixes.clear();
        frame.previews.clear();
        frame.cohort_sizes.clear();
        frame.pending_asset_events = enabled && revision == AssetRevision::Pending;
        if !enabled || revision == AssetRevision::Invalidated {
            frame.histories.clear();
        }
        frame.epoch = frame.epoch.wrapping_add(1);
        let epoch = frame.epoch;
        frame
            .histories
            .retain(|_, history| epoch.wrapping_sub(history.epoch) <= 1);
        let Frame {
            histories,
            cohort_sizes,
            ..
        } = &mut *frame;
        for history in histories.values() {
            *cohort_sizes.entry(history.lineage).or_default() += 1;
        }
        // Share live work within an ECS batch to avoid waking one task per follower.
        if enabled && revision == AssetRevision::Stable {
            reused + 1
        } else {
            1
        }
    }

    fn prefix_entry(
        &self,
        ctx: &NodeContext,
        actor: Actor,
        policy: ActorScope,
        source: &SourceAnimationInputs,
        update: &TimeUpdate,
    ) -> Option<(Arc<PrefixEntry>, bool)> {
        let mut frame = self.0.lock().unwrap();
        let current = ctx.graph_context.frame_index();
        let previous = if current <= 1 {
            0
        } else {
            let history = frame.histories.get(&actor)?;
            if history.frame.wrapping_add(1) != current {
                return None;
            }
            let previous = history.lineage;
            if frame.cohort_sizes.get(&previous) == Some(&1) {
                frame.histories.remove(&actor);
                return None;
            }
            previous
        };
        let key = Key::new(ctx, previous, update)?;
        let globals = ctx.graph_context.global_input_data;
        let foot = foot_signature(ctx, source);
        if let Some(entry) = frame
            .prefixes
            .iter()
            .find(|entry| entry.key == key && entry.inputs.matches(&policy, source, globals, foot))
        {
            return Some((Arc::clone(entry), frame.pending_asset_events));
        }
        frame.next_lineage = frame.next_lineage.wrapping_add(1);
        let entry = Arc::new(PrefixEntry {
            key,
            lineage: frame.next_lineage,
            inputs: Inputs {
                policy,
                source: source.clone(),
                globals: globals.clone(),
                foot_transition: foot,
            },
            captured: OnceLock::new(),
        });
        frame.prefixes.push(Arc::clone(&entry));
        Some((entry, frame.pending_asset_events))
    }

    fn preview_entry(
        &self,
        ctx: &NodeContext,
        policy: ActorScope,
        source: &SourceAnimationInputs,
    ) -> Option<Arc<PreviewEntry>> {
        let arena = ctx.graph_context.context_arena.get_ref();
        let current = arena.frame_index();
        let update = ctx
            .graph_context
            .context()
            .query_output_time
            .get(ctx.graph_context.state_key)
            .or_else(|| {
                arena
                    .get_toplevel()
                    .query_output_time
                    .get(StateKey::Default)
            })?;
        let mut frame = self.0.lock().unwrap();
        if frame.pending_asset_events {
            return None;
        }
        let mut histories = frame.histories.iter().filter(|((entity, ..), history)| {
            *entity == policy.entity
                && (history.frame == current || history.frame.wrapping_add(1) == current)
        });
        let (_, previous) = histories.next()?;
        if histories.next().is_some() || frame.cohort_sizes.get(&previous.lineage) == Some(&1) {
            return None;
        }
        let key = Key::new(ctx, previous.lineage, &update)?;
        let globals = ctx.graph_context.global_input_data;
        let foot = foot_signature(ctx, source);
        if let Some(entry) = frame.previews.iter().find(|entry| {
            entry.key == key
                && entry.inputs.matches(&policy, source, globals, foot)
                && entry
                    .layout
                    .iter()
                    .map(|(graph, parent)| (*graph, parent.as_ref()))
                    .eq(context_layout(arena))
        }) {
            return Some(Arc::clone(entry));
        }
        let entry = Arc::new(PreviewEntry {
            key,
            layout: context_layout(arena)
                .map(|(graph, parent)| (graph, parent.cloned()))
                .collect(),
            inputs: Inputs {
                policy,
                source: source.clone(),
                globals: globals.clone(),
                foot_transition: foot,
            },
            ticks: OnceLock::new(),
        });
        frame.previews.push(Arc::clone(&entry));
        Some(entry)
    }

    fn finish_prefix(&self, actor: Actor, entry: &PrefixEntry, succeeded: bool, reused: bool) {
        let mut frame = self.0.lock().unwrap();
        if succeeded {
            frame.reused_players += usize::from(reused);
            let epoch = frame.epoch;
            frame.histories.insert(
                actor,
                History {
                    frame: entry.key.frame,
                    epoch,
                    lineage: entry.lineage,
                },
            );
        } else {
            frame.histories.remove(&actor);
        }
    }
}

fn foot_signature(ctx: &NodeContext, inputs: &SourceAnimationInputs) -> Option<FootTransition> {
    let scalar = |name, default| {
        source_input_data(ctx, name).map_or(Some(default), |value| value.as_f32().ok())
    };
    let scale = scalar("locomotion_state.scale_world_space", 1.0)?;
    let left = scalar("feet_state.left.lock_amount", 0.0)?;
    let right = scalar("feet_state.right.lock_amount", 0.0)?;
    // The stock graph reads these frame inputs without assigning to them. The
    // stance remains part of the key, so the standing variants identify the side.
    inputs.pre_rig_foot_transition(scale, [left, right])
}

fn same_globals(a: &Globals, b: &Globals, compare_foot_result: bool) -> bool {
    a.len() == b.len()
        && a.iter().all(|(key, left)| {
            let Some(right) = b.get(key) else {
                return false;
            };
            // The stock source refresh consumes these positions through the selected
            // transition. The rig receives per-character world inputs later in the frame.
            (compare_foot_result
                && matches!(
                    key.as_str(),
                    "feet_state.left.lock_location"
                        | "feet_state.left.target_location"
                        | "feet_state.right.lock_location"
                        | "feet_state.right.target_location"
                ))
                || same_value(left, right)
        })
}

fn same_value(left: &DataValue, right: &DataValue) -> bool {
    fn vec3_bits(value: bevy::math::Vec3) -> [u32; 3] {
        value.to_array().map(f32::to_bits)
    }

    fn quat_bits(value: bevy::math::Quat) -> [u32; 4] {
        value.to_array().map(f32::to_bits)
    }

    match (left, right) {
        (DataValue::F32(a), DataValue::F32(b)) => a.to_bits() == b.to_bits(),
        (DataValue::Vec2(a), DataValue::Vec2(b)) => {
            a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)
        }
        (DataValue::Vec3(a), DataValue::Vec3(b)) => vec3_bits(*a) == vec3_bits(*b),
        (DataValue::Quat(a), DataValue::Quat(b)) => quat_bits(*a) == quat_bits(*b),
        (DataValue::BoneMask(a), DataValue::BoneMask(b)) => {
            a.base() == b.base()
                && a.get_paths() == b.get_paths()
                && a.get_weights().len() == b.get_weights().len()
                && a.get_weights().iter().all(|(bone, weight)| {
                    b.get_weights()
                        .get(bone)
                        .is_some_and(|other| other.to_bits() == weight.to_bits())
                })
        }
        (DataValue::Pose(a), DataValue::Pose(b)) => {
            a.timestamp.to_bits() == b.timestamp.to_bits()
                && a.skeleton == b.skeleton
                && a.paths == b.paths
                && a.root_motion
                    .as_ref()
                    .map(|value| (vec3_bits(value.translation), quat_bits(value.rotation)))
                    == b.root_motion
                        .as_ref()
                        .map(|value| (vec3_bits(value.translation), quat_bits(value.rotation)))
                && a.bones.len() == b.bones.len()
                && a.bones.iter().zip(&b.bones).all(|(a, b)| {
                    a.translation.map(vec3_bits) == b.translation.map(vec3_bits)
                        && a.rotation.map(quat_bits) == b.rotation.map(quat_bits)
                        && a.scale.map(vec3_bits) == b.scale.map(vec3_bits)
                        && match (&a.weights, &b.weights) {
                            (Some(a), Some(b)) => a
                                .iter()
                                .map(|v| v.to_bits())
                                .eq(b.iter().map(|v| v.to_bits())),
                            (None, None) => true,
                            _ => false,
                        }
                })
        }
        (DataValue::EventQueue(a), DataValue::EventQueue(b)) => {
            a.events.is_empty() && b.events.is_empty()
        }
        _ => left == right,
    }
}

pub(crate) fn source_pose(
    ctx: &NodeContext,
    pin: &str,
    update: &TimeUpdate,
) -> Result<(Pose, NotifyAliases), GraphError> {
    let evaluate = || ctx.data_back(pin)?.into_pose();
    let evaluate_locally = || evaluate().map(|pose| (pose, Vec::new()));
    let Some(policy) = actor_scope(ctx).filter(|_| !source_sync_preview(ctx)) else {
        return evaluate_locally();
    };
    let Some((samples, inputs)) = active_pre_rig_inputs(policy.entity) else {
        return evaluate_locally();
    };
    let arena = &ctx.graph_context.context_arena;
    let actor = (policy.entity, ctx.graph_context.context_id, ctx.node_id);
    let sharing = &samples.pre_rig_sharing;
    let Some((entry, pending_asset_events)) =
        sharing.prefix_entry(ctx, actor, policy, &inputs, update)
    else {
        return evaluate_locally();
    };
    if pending_asset_events {
        // Independently advance each actor while its cohort is still provisional.
        let result = evaluate_locally();
        sharing.finish_prefix(actor, &entry, result.is_ok(), false);
        return result;
    }
    let mut local = None;
    let shared = entry.captured.get_or_init(|| {
        let before = arena.get_ref().clone();
        let result = evaluate();
        let captured = result.as_ref().ok().and_then(|pose| {
            let writes = GraphPrefixWrites::collect(arena.get_ref(), &before)?;
            if pose_input_context(ctx, pin).is_some_and(|context| {
                context
                    .asset_samples
                    .iter()
                    .any(|sample| !writes.has_notify_owner(sample.notify_instance_id))
            }) {
                return None;
            }
            Some(CapturedPrefix {
                writes,
                pose: pose.clone(),
            })
        });
        local = Some(result);
        captured
    });
    if let Some(result) = local {
        sharing.finish_prefix(actor, &entry, result.is_ok(), false);
        return result.map(|pose| (pose, Vec::new()));
    }
    if let Some(shared) = shared
        && let Some(aliases) = shared.writes.apply(arena.get_mut())
    {
        sharing.finish_prefix(actor, &entry, true, true);
        return Ok((shared.pose.clone(), aliases));
    }
    let result = evaluate_locally();
    sharing.finish_prefix(actor, &entry, result.is_ok(), false);
    result
}

pub(crate) fn sync_preview_ticks(
    ctx: &NodeContext,
    evaluate: impl FnOnce() -> Result<(Vec<SourceSyncPlayerTick>, bool), GraphError>,
) -> Result<Vec<SourceSyncPlayerTick>, GraphError> {
    let Some(policy) = actor_scope(ctx) else {
        return evaluate().map(|(ticks, _)| ticks);
    };
    let Some((samples, inputs)) = active_pre_rig_inputs(policy.entity) else {
        return evaluate().map(|(ticks, _)| ticks);
    };
    let Some(entry) = samples.pre_rig_sharing.preview_entry(ctx, policy, &inputs) else {
        return evaluate().map(|(ticks, _)| ticks);
    };
    let mut local = None;
    let mut evaluate = Some(evaluate);
    let shared = entry.ticks.get_or_init(|| {
        let result = evaluate.take().unwrap()();
        let captured = result
            .as_ref()
            .ok()
            .and_then(|(ticks, same_layout)| same_layout.then(|| ticks.clone()));
        local = Some(result);
        captured
    });
    if let Some(result) = local {
        return result.map(|(ticks, _)| ticks);
    }
    if let Some(ticks) = shared {
        return Ok(ticks.clone());
    }
    evaluate.take().unwrap()().map(|(ticks, _)| ticks)
}

pub(crate) fn same_preview_layout(ctx: &NodeContext, snapshot: &GraphContextArena) -> bool {
    actor_scope(ctx).is_some()
        && context_layout(ctx.graph_context.context_arena.get_ref()).eq(context_layout(snapshot))
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::{AssetApp, AssetPlugin, AssetServer, Handle},
        prelude::{App, MinimalPlugins, Transform, Vec3},
    };

    use super::*;

    #[test]
    fn animation_batch_uses_recent_replays() {
        let sharing = PreRigSharing::default();
        assert_eq!(sharing.begin_frame(true, AssetRevision::Stable), 1);
        sharing.0.lock().unwrap().reused_players = 11;
        assert_eq!(sharing.begin_frame(true, AssetRevision::Stable), 12);
        sharing.0.lock().unwrap().reused_players = 7;
        assert_eq!(sharing.begin_frame(true, AssetRevision::Invalidated), 1);
        assert_eq!(sharing.begin_frame(true, AssetRevision::Stable), 1);
    }

    #[test]
    fn populated_pose_inputs_match_only_with_identical_channels() {
        use bevy_animation_graph::core::{animation_clip::EntityPath, pose::BonePose};

        let mut a = Pose::default();
        a.add_bone(
            BonePose::from_transform(Transform::IDENTITY),
            EntityPath::default().child("root").id(),
        );
        let mut b = a.clone();
        assert!(same_value(
            &DataValue::Pose(a.clone()),
            &DataValue::Pose(b.clone())
        ));

        b.bones[0].translation = Some(Vec3::new(-0.0, 0.0, 0.0));
        assert!(!same_value(&DataValue::Pose(a), &DataValue::Pose(b)));
    }

    #[test]
    fn stock_player_pins_participate_in_sharing_compatibility() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AnimationGraph>();
        crate::builtin_assets::install(&mut app);
        let root = app
            .world()
            .resource::<AssetServer>()
            .load(crate::als_asset_path(
                "ALS/Character/AB_Als.runtime.animgraph.ron",
            ));
        let mut left = AnimationGraphPlayer::new(Handle::default()).with_graph(root.clone());
        let mut right = AnimationGraphPlayer::new(Handle::default()).with_graph(root);
        let first = app.world_mut().spawn_empty().id();
        let second = app.world_mut().spawn_empty().id();
        for player in [&mut left, &mut right] {
            player.set_input_data("ragdoll_dynamic", DataValue::Bool(false));
            player.set_input_data("ragdoll_readback", DataValue::Bool(false));
            player.set_input_data("turn_weight", DataValue::F32(0.0));
        }
        let left = ActorScope::for_player(first, &left, None).unwrap();
        assert!(left.matches_policy(&ActorScope::for_player(second, &right, None).unwrap()));
        right.set_input_data("turn_weight", DataValue::F32(-0.0));
        assert!(!left.matches_policy(&ActorScope::for_player(second, &right, None).unwrap()));
    }
}
