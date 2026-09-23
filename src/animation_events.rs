use std::collections::{HashMap, HashSet};

use als_camera::AlsCameraShakeRequest;
use als_core::character::AlsCharacter;
use als_core::mantle::MantleKind;
use als_core::roll::{AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE};
use als_core::state::Stance;
use als_core::transition::AlsPhysicalStanceOverride;
use als_ext::{
    EventClock, EventDispatchContext, EventLifecycle, EventPhase, PlaybackDirection,
    StateIntervalBoundary, TimedEvent, TimedEventKind,
};
use bevy::asset::{Asset, AssetLoader, AssetServer, Assets, Handle, LoadContext, io::Reader};
use bevy::ecs::message::{Message, MessageWriter};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy_animation_graph::core::{
    animation_clip::GraphClip, edge_data::events::EventQueue, event_track::sample_tracks,
    plugin::AnimationGraphSet,
};
use serde::Deserialize;
use serde_json::Value;

use crate::get_up::{AlsGetUpState, get_up_settings};
use crate::grounded_layer::AlsGroundedEntryMode;
use crate::mantle_drive::AlsMantleDriveSet;
use crate::roll_drive::AlsRollDriveSet;

pub const ALS_NOTIFY_MANIFEST_PATH: &str = "animation_events/als_notify_manifest.json";

const SOURCE_ROLL_SEQUENCE: &str = "/ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll";
const SOURCE_ROLL_MONTAGE: &str = "/ALS/ALS/Animations/Actions/Roll/AM_Als_Roll.AM_Als_Roll";
const SOURCE_SPRINT: &str = "/ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint";
const SOURCE_SPRINT_ACCELERATION: &str =
    "/ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration";
const ROLL_CROUCH_ON_START: bool = true;

#[derive(Asset, TypePath, Deserialize, Clone, Debug)]
pub struct AlsNotifyManifest {
    pub version: u32,
    pub clips: Vec<AlsNotifyManifestClip>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AlsNotifyManifestClip {
    pub source_asset: String,
    pub source_kind: String,
    pub sequence_length: f32,
    pub runtime_segments: Vec<AlsNotifyRuntimeSegment>,
    pub events: Vec<AlsNotifyManifestEvent>,
    pub sync_markers: Vec<AlsNotifyManifestMarker>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AlsNotifyRuntimeSegment {
    pub graph_clip: String,
    pub slot: Option<String>,
    pub start_position: f32,
    pub animation_start_time: f32,
    pub animation_end_time: f32,
    pub play_rate: f32,
    pub loop_count: u32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AlsNotifyManifestEvent {
    pub id: String,
    pub event_id: String,
    pub kind: String,
    pub name: String,
    #[serde(rename = "class")]
    pub class_name: String,
    pub track: String,
    pub track_index: i32,
    pub start_time: f32,
    pub end_time: f32,
    #[serde(default)]
    pub trigger_time_offset: f32,
    pub weight_threshold: f32,
    pub filter: AlsNotifyManifestFilter,
    pub parameters: Value,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AlsNotifyManifestFilter {
    #[serde(rename = "type")]
    pub filter_type: String,
    pub lod: i32,
    pub trigger_on_follower: bool,
    pub trigger_on_dedicated_server: bool,
    pub trigger_chance: f32,
    pub montage_tick_type: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AlsNotifyManifestMarker {
    pub id: String,
    pub name: String,
    pub time: f32,
    pub track_index: i32,
}

#[derive(Default, TypePath)]
pub struct AlsNotifyManifestLoader;

impl AssetLoader for AlsNotifyManifestLoader {
    type Asset = AlsNotifyManifest;
    type Settings = ();
    type Error = AlsNotifyManifestLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

#[derive(Debug)]
pub enum AlsNotifyManifestLoadError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for AlsNotifyManifestLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "notify manifest io error: {error}"),
            Self::Json(error) => write!(formatter, "notify manifest json error: {error}"),
        }
    }
}

impl std::error::Error for AlsNotifyManifestLoadError {}

impl From<std::io::Error> for AlsNotifyManifestLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for AlsNotifyManifestLoadError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AlsStandingEventClock {
    pub sprint_time: Option<f32>,
    pub sprint_weight: f32,
    pub sprint_acceleration_weight: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlsAnimationEventPlayback {
    pub instance_id: u64,
    pub interrupted: bool,
    pub source_asset: String,
    pub graph_clip: String,
    pub time: f32,
    pub weight: f32,
    pub looped: bool,
}

#[derive(Component, Clone, Debug, Default)]
pub struct AlsAnimationEventPlaybacks {
    pub entries: Vec<AlsAnimationEventPlayback>,
}

pub use als_core::state::AlsNotifyLocomotionAction;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsNotifyRootMotionScale(pub f32);

impl Default for AlsNotifyRootMotionScale {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsNotifyEarlyBlendOut {
    pub active: bool,
    pub blend_out_duration: f32,
}

#[derive(Message, Clone, Debug)]
pub struct AlsAnimationEvent {
    pub character: Entity,
    pub instance_id: u64,
    pub source_asset: String,
    pub graph_clip: String,
    pub event_id: String,
    pub class_name: String,
    pub name: String,
    pub phase: EventPhase,
    pub time: f32,
    pub weight: f32,
    pub percentage: f32,
    pub parameters: Value,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlsAnimationEventSet {
    Refresh,
    Dispatch,
    GraphFootsteps,
}

#[derive(Resource)]
struct AlsNotifyManifestHandle(Handle<AlsNotifyManifest>);

#[derive(Resource, Default)]
struct AlsNotifyGraphClipHandles(HashMap<String, Handle<GraphClip>>);

#[derive(Default)]
struct AlsAnimationEventSourceRuntime {
    previous_time: Option<f32>,
    lifecycle: EventLifecycle,
}

#[derive(Component, Default)]
struct AlsAnimationEventRuntime {
    sources: HashMap<(String, u64), AlsAnimationEventSourceRuntime>,
}

#[derive(Default)]
struct GraphFootstepDispatch {
    frame: Option<(u64, u64)>,
    previous: HashMap<String, (f32, f32)>,
    slots: std::collections::HashSet<String>,
}

fn graph_footstep_clocks(clock: &als_ext::SourceRelevantAsset, previous_time: f32) -> Vec<EventClock> {
    let single = EventClock {
        previous_time, current_time: clock.position, duration: clock.duration,
        direction: PlaybackDirection::Forward,
        wrapped: clock.looping && clock.position < previous_time,
    };
    if !clock.looping || clock.loops_crossed == 0 || clock.duration <= 0.0 {
        return vec![single];
    }
    let mut remaining = clock.loops_crossed as f32 * clock.duration
        + clock.position - previous_time;
    if remaining <= 0.0 {
        return vec![single];
    }
    let max_segments = if remaining > clock.duration {
        ((remaining / clock.duration) as u32).clamp(2, 1000)
    } else { 2 };
    let mut position = previous_time;
    let mut segments = Vec::with_capacity(max_segments as usize);
    for _ in 0..max_segments {
        let end = (position + remaining).min(clock.duration);
        if end > position {
            segments.push(EventClock {
                previous_time: position, current_time: end,
                duration: clock.duration, direction: PlaybackDirection::Forward, wrapped: false,
            });
        }
        remaining -= end - position;
        if remaining <= 0.0 { break; }
        position = 0.0;
    }
    segments
}

fn dispatch_graph_footsteps(
    source_inputs: Query<&als_ext::SourceAnimationInputs>,
    manifest_handle: Option<Res<AlsNotifyManifestHandle>>,
    manifests: Res<Assets<AlsNotifyManifest>>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    server: Res<AssetServer>,
    avatars: Query<(&crate::character_assembly::AlsCharacterAvatar, &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance)>,
    players: Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
    mut frames: Local<HashMap<Entity, GraphFootstepDispatch>>,
    mut events: MessageWriter<AlsAnimationEvent>,
) {
    use bevy_animation_graph::core::{animation_graph::NodeId, animation_graph_player::AnimationSource, context::node_states::StateKey};
    use als_ext::sync_group_phase::{source_sync_players, select_source_sync_group_leader, SourceSyncGroupCandidate};
    let Some(manifest) = manifest_handle.as_ref().and_then(|handle| manifests.get(&handle.0)) else { return; };
    for (avatar, instance) in &avatars {
        let entity = instance.player_entity();
        let Ok(player) = players.get(entity) else { continue; };
        if player.is_paused() || player.get_error().is_some() { continue; }
        let AnimationSource::Graph(handle) = player.get_animation_source() else { continue; };
        let Some(graph) = graphs.get(handle) else { continue; };
        let Some(arena) = player.get_context_arena() else { continue; };
        let Some(node) = crate::animation_context_projection::root_pose_source_node(graph) else { continue; };
        let Some(context) = als_ext::animation_context::node_context_state(&arena.get_toplevel().node_states, StateKey::Default, node) else { continue; };
        let source_instance = arena.get_toplevel().node_states
            .get::<als_ext::source_instance::SourceInstanceProperties>(NodeId::default(), StateKey::Default)
            .map_or(0, |state| state.instance_id);
        let state = frames.entry(entity).or_default();
        let frame = (source_instance, arena.frame_index());
        if state.frame == Some(frame) { continue; }
        state.frame = Some(frame);
        let current_slots = context.asset_samples.iter().filter(|sample| sample.montage && !sample.slot_key.is_empty())
            .map(|sample| sample.slot_key.clone()).collect::<std::collections::HashSet<_>>();
        let mut samples = context.asset_samples.clone();
        for slot in state.slots.difference(&current_slots) {
            if let Some(context) = source_inputs.get(entity).ok().and_then(|inputs| inputs.slot_input_context(slot)) {
                als_ext::animation_context::merge_asset_samples(&mut samples, &context.asset_samples, 1.0);
            }
        }
        state.slots = current_slots;
        let mut active = std::collections::HashSet::new();
        for sample in &samples {
            active.insert(sample.identity.clone());
            let previous = state.previous.insert(sample.identity.clone(), (sample.clock.position, sample.clock.weight));
            let Some(path) = server.get_path(sample.clip.id()) else { continue; };
            let path = path.to_string();
            let Some(source) = manifest.clips.iter().find(|source| source.source_kind == "AnimSequence"
                && source.runtime_segments.iter().any(|segment| segment.graph_clip == path)) else { continue; };
            let previous_time = if sample.clock.has_previous_position { sample.clock.previous_position }
                else { previous.map_or(sample.clock.position, |previous| previous.0) };
            if !sample.clock.looping && sample.clock.position < previous_time { continue; }
            let is_leader = if sample.sync_group.is_empty() { true } else {
                source_sync_players(arena, &graphs, &sample.sync_group).is_some_and(|players| {
                    let players = players.iter().filter(|player| player.key == sample.sync_group).collect::<Vec<_>>();
                    let candidates = players.iter().enumerate().map(|(index, player)| SourceSyncGroupCandidate {
                        role: player.role, blend_weight: player.weight, source_order: index, marker_sync_valid: player.markers.len() >= 2,
                    }).collect::<Vec<_>>();
                    select_source_sync_group_leader(&candidates, players.first().is_some_and(|player| !player.phase_markers.is_empty()))
                        .is_some_and(|index| Some(players[index].identity) == sample.sync_identity)
                })
            };
            let definitions = timed_events(source).into_iter().filter(|definition| source.events.iter().any(|event|
                event.event_id == definition.id && event.class_name == "AlsAnimNotify_FootstepEffects")).collect::<Vec<_>>();
            let weight = if sample.montage { sample.clock.weight.max(previous.map_or(0.0, |previous| previous.1)) } else { sample.clock.weight };
            let mut signals = Vec::new();
            for clock in graph_footstep_clocks(&sample.clock, previous_time) {
                signals.extend(EventLifecycle::default().update_with_context(
                    &definitions, &EventQueue::default(), Some(clock), weight,
                    EventDispatchContext { is_leader, is_dedicated_server: false },
                ));
            }
            for signal in signals {
                let event = source.events.iter().find(|event| event.event_id == signal.id).unwrap();
                events.write(AlsAnimationEvent {
                    character: avatar.character,
                    instance_id: if sample.montage { sample.identity.rsplit(':').next().and_then(|value| value.parse().ok()).unwrap_or(0) } else { source_instance },
                    source_asset: source.source_asset.clone(), graph_clip: path.clone(), event_id: signal.id,
                    class_name: event.class_name.clone(), name: event.name.clone(), phase: signal.phase,
                    time: sample.clock.position, weight: signal.weight, percentage: signal.percentage, parameters: event.parameters.clone(),
                });
            }
        }
        state.previous.retain(|identity, _| active.contains(identity));
    }
    frames.retain(|entity, _| players.contains(*entity));
}

pub struct AlsAnimationEventPlugin;

impl Plugin for AlsAnimationEventPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<AlsNotifyManifest>()
            .register_asset_loader(AlsNotifyManifestLoader)
            .add_message::<AlsAnimationEvent>()
            .init_resource::<AlsNotifyGraphClipHandles>()
            .add_systems(Startup, load_notify_manifest)
            .configure_sets(
                PostUpdate,
                (
                    AlsAnimationEventSet::Refresh,
                    AlsAnimationEventSet::Dispatch,
                )
                    .chain()
                    .after(AnimationGraphSet::PrePhysics)
                    .before(AlsRollDriveSet)
                    .after(AlsMantleDriveSet),
            )
            .add_systems(
                PostUpdate,
                ensure_animation_event_components.before(AlsAnimationEventSet::Refresh),
            )
            .add_systems(
                PostUpdate,
                refresh_animation_event_playbacks.in_set(AlsAnimationEventSet::Refresh),
            )
            .add_systems(
                PostUpdate,
                dispatch_animation_events.in_set(AlsAnimationEventSet::Dispatch),
            )
            .configure_sets(PostUpdate, AlsAnimationEventSet::GraphFootsteps.after(AnimationGraphSet::Final))
            .add_systems(
                PostUpdate,
                dispatch_graph_footsteps.in_set(AlsAnimationEventSet::GraphFootsteps),
            );
    }
}

fn load_notify_manifest(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(AlsNotifyManifestHandle(
        assets.load(ALS_NOTIFY_MANIFEST_PATH),
    ));
}

fn ensure_animation_event_components(
    mut commands: Commands,
    characters: Query<
        (
            Entity,
            Option<&AlsAnimationEventPlaybacks>,
            Option<&AlsAnimationEventRuntime>,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsNotifyRootMotionScale>,
            Option<&AlsNotifyEarlyBlendOut>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, playbacks, runtime, action, root_scale, early_blend_out) in &characters {
        let mut entity_commands = commands.entity(entity);
        if playbacks.is_none() {
            entity_commands.insert(AlsAnimationEventPlaybacks::default());
        }
        if runtime.is_none() {
            entity_commands.insert(AlsAnimationEventRuntime::default());
        }
        if action.is_none() {
            entity_commands.insert(AlsNotifyLocomotionAction::None);
        }
        if root_scale.is_none() {
            entity_commands.insert(AlsNotifyRootMotionScale::default());
        }
        if early_blend_out.is_none() {
            entity_commands.insert(AlsNotifyEarlyBlendOut::default());
        }
    }
}

fn push_playback(
    entries: &mut Vec<AlsAnimationEventPlayback>,
    source_asset: &str,
    graph_clip: &str,
    time: f32,
    weight: f32,
    looped: bool,
    interrupted: bool,
) {
    if !time.is_finite() || !weight.is_finite() || weight <= f32::EPSILON {
        return;
    }
    entries.push(AlsAnimationEventPlayback {
        instance_id: 0,
        interrupted,
        source_asset: source_asset.into(),
        graph_clip: graph_clip.into(),
        time,
        weight: weight.clamp(0.0, 1.0),
        looped,
    });
}

fn mantle_sources(kind: MantleKind) -> (&'static str, &'static str) {
    let settings = crate::mantle_drive::mantle_montage_settings(kind);
    (&settings.source_sequence, &settings.source_montage)
}

fn graph_clip_for_source<'a>(
    manifest: &'a AlsNotifyManifest,
    source_asset: &str,
) -> Option<&'a str> {
    manifest
        .clips
        .iter()
        .find(|clip| clip.source_asset == source_asset)
        .and_then(|clip| clip.runtime_segments.first())
        .map(|segment| segment.graph_clip.as_str())
}

fn refresh_animation_event_playbacks(
    manifest_handle: Option<Res<AlsNotifyManifestHandle>>,
    manifests: Res<Assets<AlsNotifyManifest>>,
    mut characters: Query<
        (
            &mut AlsAnimationEventPlaybacks,
            Option<&AlsStandingEventClock>,
            Option<&AlsRollState>,
            Option<&crate::roll_drive::AlsRollBlendOut>,
            Option<&crate::mantle_drive::AlsMantleMontages>,
            Option<&AlsGetUpState>,
        ),
        With<AlsCharacter>,
    >,
) {
    let Some(handle) = manifest_handle else {
        return;
    };
    let Some(manifest) = manifests.get(&handle.0) else {
        return;
    };
    for (mut playbacks, standing, roll, roll_blend_out, mantle, get_up) in &mut characters {
        let mut entries = Vec::new();
        if let Some(clock) = standing {
            let time = clock.sprint_time.unwrap_or_default();
            let base_weight = clock.sprint_weight - clock.sprint_acceleration_weight;
            if let Some(graph_clip) = graph_clip_for_source(manifest, SOURCE_SPRINT) {
                push_playback(
                    &mut entries,
                    SOURCE_SPRINT,
                    graph_clip,
                    time,
                    base_weight,
                    true,
                    false,
                );
            }
            if let Some(graph_clip) = graph_clip_for_source(manifest, SOURCE_SPRINT_ACCELERATION) {
                push_playback(
                    &mut entries,
                    SOURCE_SPRINT_ACCELERATION,
                    graph_clip,
                    time,
                    clock.sprint_acceleration_weight,
                    true,
                    false,
                );
            }
        }
        if let Some(roll) = roll.filter(|state| state.active) {
            let time = (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
            if let Some(graph_clip) = graph_clip_for_source(manifest, SOURCE_ROLL_SEQUENCE) {
                push_playback(
                    &mut entries,
                    SOURCE_ROLL_SEQUENCE,
                    graph_clip,
                    time,
                    1.0,
                    false,
                    roll_blend_out.is_some(),
                );
            }
            if let Some(graph_clip) = graph_clip_for_source(manifest, SOURCE_ROLL_MONTAGE) {
                push_playback(
                    &mut entries,
                    SOURCE_ROLL_MONTAGE,
                    graph_clip,
                    time,
                    1.0,
                    false,
                    roll_blend_out.is_some(),
                );
            }
        }
        if let Some(mantle) = mantle {
            for entry in mantle.entries.iter().filter(|entry| entry.weight > f32::EPSILON) {
                let (sequence, montage) = mantle_sources(entry.kind);
                for source in [sequence, montage] {
                    if let Some(graph_clip) = graph_clip_for_source(manifest, source) {
                        push_playback(&mut entries, source, graph_clip, entry.clip_time, entry.weight, false, entry.interrupted);
                        let playback = entries.last_mut().unwrap();
                        playback.instance_id = entry.id;
                    }
                }
            }
        }
        if let Some(get_up) = get_up {
            let settings = get_up_settings(get_up.facing_up);
            for source in [&settings.source_sequence, &settings.source_montage] {
                if let Some(graph_clip) = graph_clip_for_source(manifest, source) {
                    push_playback(
                        &mut entries,
                        source,
                        graph_clip,
                        get_up.clip_time,
                        get_up.weight,
                        false,
                        get_up.interrupted,
                    );
                }
            }
        }
        playbacks.entries = entries;
    }
}

fn event_kind(event: &AlsNotifyManifestEvent) -> TimedEventKind {
    match event.kind.as_str() {
        "state" => TimedEventKind::State,
        _ => TimedEventKind::Point,
    }
}

fn event_boundary(event: &AlsNotifyManifestEvent) -> StateIntervalBoundary {
    match event.class_name.as_str() {
        "AlsAnimNotifyState_SetLocomotionAction" | "AlsAnimNotifyState_EarlyBlendOut" => {
            StateIntervalBoundary::StartExclusiveEndInclusive
        }
        _ => StateIntervalBoundary::StartInclusiveEndExclusive,
    }
}

fn timed_events(clip: &AlsNotifyManifestClip) -> Vec<TimedEvent> {
    let mut events = clip
        .events
        .iter()
        .map(|event| TimedEvent {
            id: event.event_id.clone(),
            kind: event_kind(event),
            start_time: event.start_time + if event.kind == "point" { event.trigger_time_offset } else { 0.0 },
            end_time: event.end_time,
            weight_threshold: event.weight_threshold,
            track: Some(event.track.clone()),
            state_boundary: event_boundary(event),
            trigger_on_follower: event.filter.trigger_on_follower,
            trigger_on_dedicated_server: event.filter.trigger_on_dedicated_server,
        })
        .collect::<Vec<_>>();
    events.extend(clip.sync_markers.iter().map(|marker| TimedEvent {
        id: marker_event_id(&clip.source_asset, &marker.id),
        kind: TimedEventKind::Marker,
        start_time: marker.time,
        end_time: marker.time,
        weight_threshold: 0.0,
        track: Some(format!("sync_marker:{}", marker.track_index)),
        state_boundary: StateIntervalBoundary::StartInclusiveEndExclusive,
        trigger_on_follower: false,
        trigger_on_dedicated_server: false,
    }));
    events
}

fn marker_event_id(source_asset: &str, marker_id: &str) -> String {
    format!(
        "als_marker:{}:{}",
        source_asset.to_lowercase(),
        marker_id.to_lowercase()
    )
}

fn graph_clip_handle(
    cache: &mut AlsNotifyGraphClipHandles,
    assets: &AssetServer,
    graph_clip: &str,
) -> Handle<GraphClip> {
    cache
        .0
        .entry(graph_clip.into())
        .or_insert_with(|| assets.load(graph_clip.to_string()))
        .clone()
}

fn sampled_queue(graph_clip: Option<&GraphClip>, time: f32) -> EventQueue {
    graph_clip.map_or_else(EventQueue::default, |clip| {
        EventQueue::with_events(sample_tracks(clip.event_tracks.values(), time))
    })
}

fn playback_clock(
    runtime: &mut AlsAnimationEventSourceRuntime,
    playback: &AlsAnimationEventPlayback,
) -> Option<EventClock> {
    let Some(previous) = runtime.previous_time.replace(playback.time) else {
        return Some(EventClock {
            previous_time: playback.time,
            current_time: playback.time,
            duration: 0.0,
            direction: PlaybackDirection::Forward,
            wrapped: false,
        });
    };
    if !playback.looped && playback.time + 1e-5 < previous {
        runtime.previous_time = Some(playback.time);
        return None;
    }
    Some(EventClock {
        previous_time: previous,
        current_time: playback.time,
        duration: 0.0,
        direction: PlaybackDirection::Forward,
        wrapped: playback.looped && playback.time + 1e-5 < previous,
    })
}

fn source_clip<'a>(
    manifest: &'a AlsNotifyManifest,
    source_asset: &str,
) -> Option<&'a AlsNotifyManifestClip> {
    manifest
        .clips
        .iter()
        .find(|clip| clip.source_asset == source_asset)
}

pub(crate) fn parameter_f32(parameters: &Value, key: &str, default: f32) -> f32 {
    parameters
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

fn parameter_tag<'a>(parameters: &'a Value, key: &str) -> Option<&'a str> {
    parameters
        .get(key)
        .and_then(|value| value.get("__tagged__"))
        .and_then(|value| value.get("TagName"))
        .and_then(Value::as_str)
}

pub(crate) fn early_blend_out_matches(
    parameters: &Value, has_input: bool, locomotion: als_core::state::LocomotionState,
    rotation: als_core::state::RotationMode, stance: Stance,
) -> bool {
    let locomotion_tag = locomotion.as_str();
    let rotation_tag = rotation.as_str();
    let stance_tag = stance.as_str();
    [
        ("bCheckInput", has_input),
        ("bCheckLocomotionMode", locomotion_tag == parameter_tag(parameters, "LocomotionModeEquals").unwrap_or("Als.LocomotionMode.InAir")),
        ("bCheckRotationMode", rotation_tag == parameter_tag(parameters, "RotationModeEquals").unwrap_or("Als.RotationMode.Aiming")),
        ("bCheckStance", stance_tag == parameter_tag(parameters, "StanceEquals").unwrap_or("Als.Stance.Crouching")),
    ].into_iter().any(|(key, matches)| parameters.get(key).and_then(Value::as_bool).unwrap_or(true) && matches)
}

fn notify_action(tag: Option<&str>) -> AlsNotifyLocomotionAction {
    AlsNotifyLocomotionAction::new(tag.unwrap_or("")).expect("Source locomotion action must be a valid gameplay tag")
}

fn physical_stance_override(
    action: AlsNotifyLocomotionAction,
) -> Option<AlsPhysicalStanceOverride> {
    if action == AlsNotifyLocomotionAction::Rolling && ROLL_CROUCH_ON_START {
        Some(AlsPhysicalStanceOverride(Stance::Crouching))
    } else {
        None
    }
}

fn dispatch_animation_events(
    mut commands: Commands,
    manifest_handle: Option<Res<AlsNotifyManifestHandle>>,
    manifests: Res<Assets<AlsNotifyManifest>>,
    asset_server: Res<AssetServer>,
    graph_clips: Res<Assets<GraphClip>>,
    mut graph_clip_handles: ResMut<AlsNotifyGraphClipHandles>,
    mut characters: Query<
        (
            Entity,
            &AlsAnimationEventPlaybacks,
            &mut AlsAnimationEventRuntime,
            &mut AlsNotifyLocomotionAction,
            Option<&AlsPhysicalStanceOverride>,
            &mut AlsNotifyRootMotionScale,
            &mut AlsNotifyEarlyBlendOut,
        ),
        With<AlsCharacter>,
    >,
    mut events: MessageWriter<AlsAnimationEvent>,
    mut shakes: MessageWriter<AlsCameraShakeRequest>,
    avatars: Query<&crate::character_assembly::AlsCharacterAvatar>,
) {
    let Some(handle) = manifest_handle else {
        return;
    };
    let Some(manifest) = manifests.get(&handle.0) else {
        return;
    };
    for (
        character,
        playbacks,
        mut runtime,
        mut action,
        physical_override,
        mut root_scale,
        mut early_blend_out,
    ) in &mut characters
    {
        let active_sources = playbacks.entries.iter()
            .filter(|playback| source_clip(manifest, &playback.source_asset).is_some())
            .map(|playback| (playback.source_asset.clone(), playback.instance_id))
            .collect::<HashSet<_>>();
        let inactive = runtime
            .sources
            .keys()
            .filter(|source| !active_sources.contains(*source))
            .cloned()
            .collect::<Vec<_>>();
        for (source_asset, instance_id) in inactive {
            let Some(source) = source_clip(manifest, &source_asset) else {
                continue;
            };
            let source_runtime = runtime
                .sources
                .get_mut(&(source_asset.clone(), instance_id))
                .expect("source runtime must exist");
            for signal in source_runtime.lifecycle.reset() {
                let event = source
                    .events
                    .iter()
                    .find(|event| event.event_id == signal.id);
                if let Some(event) = event {
                    if event.class_name == "AlsAnimNotifyState_SetLocomotionAction" {
                        let next_action =
                            notify_action(parameter_tag(&event.parameters, "LocomotionAction"));
                        if *action == next_action {
                            *action = AlsNotifyLocomotionAction::None;
                        }
                    }
                    if event.class_name == "AlsAnimNotifyState_SetRootMotionScale" {
                        let scale = parameter_f32(&event.parameters, "TranslationScale", 1.0);
                        if (root_scale.0 - scale).abs() <= 1e-4 {
                            root_scale.0 = 1.0;
                        }
                    }
                    if event.class_name == "AlsAnimNotifyState_EarlyBlendOut" {
                        *early_blend_out = AlsNotifyEarlyBlendOut::default();
                    }
                    events.write(AlsAnimationEvent {
                        character,
                        instance_id,
                        source_asset: source_asset.clone(),
                        graph_clip: source
                            .runtime_segments
                            .first()
                            .map_or_else(String::new, |segment| segment.graph_clip.clone()),
                        event_id: signal.id,
                        class_name: event.class_name.clone(),
                        name: event.name.clone(),
                        phase: signal.phase,
                        time: source_runtime.previous_time.unwrap_or_default(),
                        weight: signal.weight,
                        percentage: signal.percentage,
                        parameters: event.parameters.clone(),
                    });
                }
            }
            runtime.sources.remove(&(source_asset, instance_id));
        }
        for playback in &playbacks.entries {
            let Some(source) = source_clip(manifest, &playback.source_asset) else {
                continue;
            };
            let graph_handle =
                graph_clip_handle(&mut graph_clip_handles, &asset_server, &playback.graph_clip);
            let queue = sampled_queue(graph_clips.get(&graph_handle), playback.time);
            let source_runtime = runtime
                .sources
                .entry((playback.source_asset.clone(), playback.instance_id))
                .or_default();
            if playback.interrupted { continue; }
            let clock = playback_clock(source_runtime, playback);
            let definitions = timed_events(source);
            let signals = source_runtime.lifecycle.update_with_context(
                &definitions,
                &queue,
                clock,
                playback.weight,
                EventDispatchContext::default(),
            );
            for signal in signals {
                let event = source
                    .events
                    .iter()
                    .find(|event| event.event_id == signal.id);
                let (class_name, name, parameters) = match event {
                    Some(event) => (
                        event.class_name.clone(),
                        event.name.clone(),
                        event.parameters.clone(),
                    ),
                    None => (
                        "AuthoredSyncMarker".into(),
                        source
                            .sync_markers
                            .iter()
                            .find(|marker| {
                                marker_event_id(&source.source_asset, &marker.id) == signal.id
                            })
                            .map_or_else(|| "None".into(), |marker| marker.name.clone()),
                        Value::Null,
                    ),
                };
                if class_name == "AlsAnimNotify_FootstepEffects" && avatars.iter().any(|avatar| avatar.character == character) {
                    continue;
                }
                if class_name == "AlsAnimNotify_CameraShake" && signal.phase == EventPhase::Point {
                    shakes.write(AlsCameraShakeRequest::sprint(
                        character,
                        parameter_f32(&parameters, "CameraShakeScale", 1.0),
                    ));
                }
                if class_name == "AlsAnimNotify_SetGroundedEntryMode"
                    && signal.phase == EventPhase::Point
                    && parameter_tag(&parameters, "GroundedEntryMode")
                        == Some("Als.GroundedEntryMode.FromRoll")
                {
                    commands
                        .entity(character)
                        .insert(AlsGroundedEntryMode::FromRoll);
                }
                if class_name == "AlsAnimNotifyState_SetLocomotionAction" {
                    let next_action = notify_action(parameter_tag(&parameters, "LocomotionAction"));
                    match signal.phase {
                        EventPhase::Begin => *action = next_action,
                        EventPhase::End if *action == next_action => {
                            *action = AlsNotifyLocomotionAction::None;
                        }
                        _ => {}
                    }
                }
                if class_name == "AlsAnimNotifyState_SetRootMotionScale" {
                    let scale = parameter_f32(&parameters, "TranslationScale", 1.0);
                    match signal.phase {
                        EventPhase::Begin | EventPhase::Active => root_scale.0 = scale,
                        EventPhase::End if (root_scale.0 - scale).abs() <= 1e-4 => {
                            root_scale.0 = 1.0;
                        }
                        _ => {}
                    }
                }
                if class_name == "AlsAnimNotifyState_EarlyBlendOut" {
                    match signal.phase {
                        EventPhase::Begin | EventPhase::Active => {
                            early_blend_out.active = true;
                            early_blend_out.blend_out_duration =
                                parameter_f32(&parameters, "BlendOutDuration", 0.25);
                        }
                        EventPhase::End => *early_blend_out = AlsNotifyEarlyBlendOut::default(),
                        _ => {}
                    }
                }
                events.write(AlsAnimationEvent {
                    character,
                    instance_id: playback.instance_id,
                    source_asset: playback.source_asset.clone(),
                    graph_clip: playback.graph_clip.clone(),
                    event_id: signal.id,
                    class_name,
                    name,
                    phase: signal.phase,
                    time: playback.time,
                    weight: signal.weight,
                    percentage: signal.percentage,
                    parameters,
                });
            }
        }
        match (physical_stance_override(action.clone()), physical_override) {
            (Some(next), Some(current)) if *current == next => {}
            (Some(next), _) => {
                commands.entity(character).insert(next);
            }
            (None, Some(_)) => {
                commands
                    .entity(character)
                    .remove::<AlsPhysicalStanceOverride>();
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_source_action_and_early_blend_tags_keep_their_identity() {
        assert_eq!(notify_action(Some("Als.LocomotionAction.Custom")).as_str(), "Als.LocomotionAction.Custom");
        let rotation = als_core::state::RotationMode::new("Als.RotationMode.Custom").unwrap();
        let stance = Stance::new("Als.Stance.Custom").unwrap();
        let parameters = serde_json::json!({"bCheckInput": false, "bCheckLocomotionMode": false,
            "bCheckRotationMode": true, "RotationModeEquals": {"__tagged__": {"TagName": rotation.as_str()}}, "bCheckStance": false});
        assert!(early_blend_out_matches(&parameters, false, als_core::state::LocomotionState::Grounded, rotation, stance));
        assert!(!early_blend_out_matches(&parameters, false, als_core::state::LocomotionState::Grounded, als_core::state::RotationMode::Aiming, stance));
    }

    #[test]
    fn graph_footstep_clocks_keep_source_loop_budget() {
        let clock = |previous, current, loops, looping| graph_footstep_clocks(
            &als_ext::SourceRelevantAsset {
                duration: 1.0, position: current, loops_crossed: loops, looping, ..default()
            }, previous);
        assert_eq!(clock(0.1, 0.2, 0, true).len(), 1);
        let wrapped = clock(0.9, 0.1, 1, true);
        assert_eq!(wrapped.len(), 2);
        assert_eq!((wrapped[0].previous_time, wrapped[0].current_time), (0.9, 1.0));
        assert_eq!(wrapped[1].previous_time, 0.0);
        assert!((wrapped[1].current_time - 0.1).abs() < 1e-6);
        assert_eq!(clock(0.1, 0.1, 2, true).len(), 2);
        assert_eq!(clock(0.1, 0.6, 2, true).len(), 2);
        assert_eq!(clock(0.1, 0.6, 2, false).len(), 1);
        assert!(graph_footstep_clocks(&als_ext::SourceRelevantAsset {
            duration: 1.0, position: 0.1, loops_crossed: 1000, looping: true, ..default()
        }, 0.1).len() <= 1000);
    }

    #[test]
    fn generated_clip_playback_dispatches_footsteps_from_its_actual_clock() {
        use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance,
            animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer,
            animation_node::AnimationNode, edge_data::DataSpec};
        let (mut app, character, avatar, _) = crate::camera_component::tests::loaded_camera_scene();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().entity_mut(avatar)
            .remove::<(crate::camera_component::AlsSourceCameraComponent, crate::character_assembly::AlsCharacterCamera)>()
            .insert(crate::character_assembly::AlsCharacterAvatar { character });
        app.world_mut().entity_mut(character).insert(AlsCharacter);
        let clip: Handle<GraphClip> = app.world().resource::<AssetServer>()
            .load("animations/Grounded/WalkRun/A_Als_Walk_Forward.anim.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while app.world().resource::<Assets<GraphClip>>().get(&clip).is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Footstep source clip load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let loaded_clip = app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap();
        let skeleton = loaded_clip.skeleton.clone();
        let clip_duration = loaded_clip.duration();
        let node = AnimationNode::new("source walk", als_ext::SourceSequencePlayerNode {
            clip, loop_animation: true, ..default()
        });
        let mut graph = AnimationGraph::default();
        graph.add_output_data("pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        graph.add_node(node);
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        app.world_mut().entity_mut(player_entity)
            .remove::<als_ext::post_physics_animation::ManualAnimationPlayer>()
            .insert(AnimationGraphPlayer::new(skeleton).with_graph(graph.clone()));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
        let mut feet = [0; 2];
        for _ in 0..150 {
            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none(), "{:?}", player.get_error());
            assert!(player.get_default_output_pose().is_some_and(|pose| !pose.paths.is_empty()));
            for event in app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain() {
                if event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects" {
                    let right = event.parameters.get("FootBone").and_then(|value| value.get("value")).and_then(|value| value.as_str()).is_some_and(|value| value.ends_with("::Right"));
                    feet[usize::from(right)] += 1;
                }
            }
        }
        assert!(feet.iter().all(|count| *count >= 2), "Both source footstep notifies must cross their actual clip clock: {feet:?}");
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().start(graph);
        app.update();
        app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
        app.world_mut().resource_mut::<Time<Virtual>>()
            .set_max_delta(std::time::Duration::from_secs_f32(3.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(clip_duration * 2.0)));
        app.update();
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        let bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(handle) =
            player.get_animation_source() else { panic!("Source graph is missing"); };
        let graph = app.world().resource::<Assets<AnimationGraph>>().get(handle).unwrap();
        let arena = player.get_context_arena().unwrap();
        let node = crate::animation_context_projection::root_pose_source_node(graph).unwrap();
        let context = als_ext::animation_context::node_context_state(&arena.get_toplevel().node_states,
            bevy_animation_graph::core::context::node_states::StateKey::Default, node).unwrap();
        assert_eq!(context.asset_samples[0].clock.loops_crossed, 2);
        let full_cycle_footsteps = app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>()
            .drain().filter(|event| event.character == character
                && event.class_name == "AlsAnimNotify_FootstepEffects").count();
        assert_eq!(full_cycle_footsteps, 4);
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().pause();
        for _ in 0..10 {
            app.update();
            assert!(!app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                .any(|event| event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects"));
        }
    }

    #[test]
    fn source_zero_frame_footsteps_keep_their_trigger_offset() {
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../assets/animation_events/als_notify_manifest.json")).unwrap();
        for side in ["Left", "Right"] {
            let source = manifest.clips.iter().find(|clip| clip.source_asset.ends_with(&format!("A_Als_Jump_Run_{side}"))).unwrap();
            let events = timed_events(source);
            let signals = EventLifecycle::default().update(&events, &EventQueue::default(), Some(EventClock {
                previous_time: 0.0, current_time: 1.0 / 60.0, duration: source.sequence_length,
                direction: PlaybackDirection::Forward, wrapped: false,
            }), 1.0);
            assert_eq!(signals.iter().filter(|signal| signal.phase == EventPhase::Point).count(), 1);
        }
    }

    #[test]
    fn mantle_notify_instances_preserve_external_actions_and_interrupted_tails() {
        use bevy::ecs::system::RunSystemOnce;
        for external_action in [
            AlsNotifyLocomotionAction::GettingUp,
            AlsNotifyLocomotionAction::Ragdolling,
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default()))
                .init_asset::<GraphClip>()
                .init_asset::<AlsNotifyManifest>()
                .init_resource::<AlsNotifyGraphClipHandles>()
                .add_message::<AlsAnimationEvent>()
                .add_message::<AlsCameraShakeRequest>();
            let manifest: AlsNotifyManifest = serde_json::from_str(include_str!(
                "../assets/animation_events/als_notify_manifest.json"
            ))
            .unwrap();
            let source = mantle_sources(MantleKind::Low).1;
            let graph_clip = graph_clip_for_source(&manifest, source).unwrap().to_owned();
            let handle = app
                .world_mut()
                .resource_mut::<Assets<AlsNotifyManifest>>()
                .add(manifest);
            app.insert_resource(AlsNotifyManifestHandle(handle));
            let mut entries = Vec::new();
            push_playback(&mut entries, source, &graph_clip, 0.3, 1.0, false, false);
            entries[0].instance_id = 1;
            let actor = app
                .world_mut()
                .spawn((AlsCharacter, AlsAnimationEventPlaybacks { entries }))
                .id();
            let world = app.world_mut();
            world
                .run_system_once(ensure_animation_event_components)
                .unwrap();
            world.run_system_once(dispatch_animation_events).unwrap();
            assert_eq!(
                world.get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Mantling)
            );
            world.entity_mut(actor).insert(external_action);
            world
                .get_mut::<AlsAnimationEventPlaybacks>(actor)
                .unwrap()
                .entries[0]
                .time = 0.4;
            world.run_system_once(dispatch_animation_events).unwrap();
            assert_eq!(
                world.get::<AlsNotifyLocomotionAction>(actor),
                Some(&external_action)
            );
            world.resource_mut::<Messages<AlsAnimationEvent>>().clear();
            {
                let mut playbacks = world.get_mut::<AlsAnimationEventPlaybacks>(actor).unwrap();
                playbacks.entries[0].interrupted = true;
                playbacks.entries[0].time = 0.6;
                let mut next = playbacks.entries[0].clone();
                next.instance_id = 2;
                next.interrupted = false;
                next.time = 0.2;
                playbacks.entries.push(next);
            }
            world.run_system_once(dispatch_animation_events).unwrap();
            assert_eq!(
                world.get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Mantling)
            );
            assert_eq!(
                world
                    .get::<AlsAnimationEventRuntime>(actor)
                    .unwrap()
                    .sources
                    .len(),
                2
            );
            let emitted: Vec<_> = world
                .resource_mut::<Messages<AlsAnimationEvent>>()
                .drain()
                .collect();
            assert!(
                emitted
                    .iter()
                    .any(|event| event.instance_id == 2 && event.phase == EventPhase::Begin)
            );
            assert!(!emitted.iter().any(|event| event.instance_id == 1));
            world.entity_mut(actor).insert(external_action);
            world
                .get_mut::<AlsAnimationEventPlaybacks>(actor)
                .unwrap()
                .entries
                .remove(0);
            world.run_system_once(dispatch_animation_events).unwrap();
            let emitted: Vec<_> = world
                .resource_mut::<Messages<AlsAnimationEvent>>()
                .drain()
                .collect();
            assert!(
                emitted
                    .iter()
                    .any(|event| event.instance_id == 1 && event.phase == EventPhase::End)
            );
            assert_eq!(
                world.get::<AlsNotifyLocomotionAction>(actor),
                Some(&external_action)
            );
            world
                .entity_mut(actor)
                .insert(AlsNotifyLocomotionAction::Mantling);
            world
                .get_mut::<AlsAnimationEventPlaybacks>(actor)
                .unwrap()
                .entries[0]
                .time = 1.05;
            world.run_system_once(dispatch_animation_events).unwrap();
            assert_eq!(
                world.get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::None)
            );
            assert!(world.get::<AlsNotifyEarlyBlendOut>(actor).unwrap().active);
        }
    }

    #[test]
    fn mantle_early_blend_out_uses_source_conditions_and_custom_movement_mode() {
        use als_core::state::{LocomotionState, RotationMode};
        let defaults = serde_json::json!({"BlendOutDuration": 0.4});
        let matches = |parameters: &Value, input, locomotion, rotation, stance| {
            early_blend_out_matches(parameters, input, locomotion, rotation, stance)
        };
        assert!(!matches(&defaults, false, LocomotionState::Grounded, RotationMode::ViewDirection, Stance::Standing));
        assert!(matches(&defaults, true, LocomotionState::Grounded, RotationMode::ViewDirection, Stance::Standing));
        assert!(matches(&defaults, false, LocomotionState::Air, RotationMode::ViewDirection, Stance::Standing));
        assert!(!matches(&defaults, false, LocomotionState::Custom, RotationMode::ViewDirection, Stance::Standing));
        assert!(matches(&defaults, false, LocomotionState::Grounded, RotationMode::Aiming, Stance::Standing));
        assert!(matches(&defaults, false, LocomotionState::Grounded, RotationMode::ViewDirection, Stance::Crouching));
        let disabled = serde_json::json!({"bCheckInput": false, "bCheckLocomotionMode": false, "bCheckRotationMode": false, "bCheckStance": false});
        assert!(!matches(&disabled, true, LocomotionState::Air, RotationMode::Aiming, Stance::Crouching));
        let configured = serde_json::json!({"LocomotionModeEquals": {"__tagged__": {"TagName": "Als.LocomotionMode.Grounded"}}});
        assert!(matches(&configured, false, LocomotionState::Grounded, RotationMode::ViewDirection, Stance::Standing));
    }

    #[test]
    fn interrupted_action_playbacks_do_not_replace_the_ragdoll_action() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<GraphClip>()
            .init_asset::<AlsNotifyManifest>()
            .init_resource::<AlsNotifyGraphClipHandles>()
            .add_message::<AlsAnimationEvent>()
            .add_message::<AlsCameraShakeRequest>();
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!(
            "../assets/animation_events/als_notify_manifest.json"
        ))
        .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<AlsNotifyManifest>>()
            .add(manifest);
        app.insert_resource(AlsNotifyManifestHandle(handle));
        let world = app.world_mut();
        let mut observed = Vec::new();
        for variant in 0..5 {
            let actor = world
                .spawn((
                    AlsCharacter,
                    AlsAnimationEventPlaybacks::default(),
                    AlsNotifyLocomotionAction::Ragdolling,
                ))
                .id();
            if variant == 0 {
                let roll = AlsRollState {
                    active: true,
                    time: 0.2,
                    ..default()
                };
                world
                    .entity_mut(actor)
                    .insert((roll, crate::roll_drive::AlsRollBlendOut::new(&roll, 0.2)));
            } else {
                let mut get_up = AlsGetUpState::new(variant % 2 == 0);
                get_up.elapsed = 0.2;
                if variant >= 3 {
                    let settings = get_up_settings(get_up.facing_up);
                    get_up.elapsed = settings.sequence_length / settings.play_rate
                        - settings.blend_out_seconds * 0.5;
                }
                get_up.advance(0.0);
                let before = get_up.weight;
                get_up.stop(if variant >= 3 { 100.0 } else { 0.2 });
                get_up.advance(0.0);
                assert_eq!(get_up.weight, before);
                world.entity_mut(actor).insert(get_up);
            }
            world
                .run_system_once(ensure_animation_event_components)
                .unwrap();
            world
                .run_system_once(refresh_animation_event_playbacks)
                .unwrap();
            world.run_system_once(dispatch_animation_events).unwrap();
            let entries = &world
                .get::<AlsAnimationEventPlaybacks>(actor)
                .unwrap()
                .entries;
            assert_eq!(entries.len(), 2);
            observed.push((
                entries.iter().all(|entry| entry.interrupted),
                *world.get::<AlsNotifyLocomotionAction>(actor).unwrap(),
            ));
            world.despawn(actor);
        }
        assert_eq!(
            observed,
            vec![(true, AlsNotifyLocomotionAction::Ragdolling); 5]
        );
    }

    #[test]
    fn get_up_playbacks_use_the_existing_sequence_and_montage_event_streams() {
        use bevy::ecs::system::RunSystemOnce;
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!(
            "../assets/animation_events/als_notify_manifest.json"
        ))
        .unwrap();
        let mut world = World::new();
        let mut manifests = Assets::<AlsNotifyManifest>::default();
        let handle = manifests.add(manifest);
        world.insert_resource(manifests);
        world.insert_resource(AlsNotifyManifestHandle(handle));
        for facing_up in [false, true] {
            let mut state = AlsGetUpState::new(facing_up);
            state.clip_time = 0.2;
            let entity = world
                .spawn((AlsCharacter, AlsAnimationEventPlaybacks::default(), state))
                .id();
            world
                .run_system_once(refresh_animation_event_playbacks)
                .unwrap();
            let entries = &world
                .get::<AlsAnimationEventPlaybacks>(entity)
                .unwrap()
                .entries;
            let settings = get_up_settings(facing_up);
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].source_asset, settings.source_sequence);
            assert_eq!(entries[1].source_asset, settings.source_montage);
            assert!(
                entries
                    .iter()
                    .all(|entry| entry.time == 0.2 && entry.graph_clip == settings.clip)
            );
            world.despawn(entity);
        }
    }

    #[test]
    fn source_paths_select_the_expected_mantle_variants() {
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../assets/animation_events/als_notify_manifest.json")).unwrap();
        for kind in [MantleKind::High, MantleKind::InAir, MantleKind::Low, MantleKind::LowLeft, MantleKind::LowRight, MantleKind::LowBothHands, MantleKind::LowBox] {
            let settings = crate::mantle_drive::mantle_montage_settings(kind);
            let (sequence, montage) = mantle_sources(kind);
            for source in [sequence, montage] {
                let record = source_clip(&manifest, source).unwrap();
                assert_eq!(record.runtime_segments.len(), 1);
                assert_eq!(record.runtime_segments[0].graph_clip, settings.clip);
            }
            let action = source_clip(&manifest, montage).unwrap().events.iter()
                .find(|event| event.class_name == "AlsAnimNotifyState_SetLocomotionAction").unwrap();
            assert_eq!(action.start_time, 0.0);
            assert!((action.end_time - if kind.is_high() { 2.066667 } else { 0.966667 }).abs() < 1e-6);
        }
        assert_ne!(mantle_sources(MantleKind::Low).1, mantle_sources(MantleKind::LowRight).1);
        assert_eq!(
            mantle_sources(MantleKind::High).0,
            "/ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.A_Als_Mantle_High"
        );
        assert_eq!(
            mantle_sources(MantleKind::LowBothHands).1,
            "/ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_BothArms.AM_Als_Mantle_Low_BothArms"
        );
        assert_eq!(
            mantle_sources(MantleKind::LowBox).1,
            "/ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Box.AM_Als_Mantle_Low_Box"
        );
    }

    #[test]
    fn locomotion_action_tags_map_to_project_actions() {
        assert_eq!(
            notify_action(Some("Als.LocomotionAction.Rolling")),
            AlsNotifyLocomotionAction::Rolling
        );
        assert_eq!(
            notify_action(Some("Als.LocomotionAction.Mantling")),
            AlsNotifyLocomotionAction::Mantling
        );
        assert_eq!(notify_action(None), AlsNotifyLocomotionAction::None);
    }

    #[test]
    fn rolling_action_uses_a_physical_crouch_override() {
        assert_eq!(
            physical_stance_override(AlsNotifyLocomotionAction::Rolling),
            Some(AlsPhysicalStanceOverride(Stance::Crouching))
        );
        assert_eq!(
            physical_stance_override(AlsNotifyLocomotionAction::None),
            None
        );
    }
}
