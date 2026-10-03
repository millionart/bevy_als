use std::collections::{HashMap, HashSet};

use crate::als_camera::AlsCameraShakeRequest;
use crate::als::character::AlsCharacter;
use crate::als::mantle::MantleKind;
use crate::als::roll::{AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE};
use crate::als::state::Stance;
use crate::als::transition::AlsPhysicalStanceOverride;
use crate::extensions::{
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

pub const ALS_NOTIFY_MANIFEST_PATH: &str = "ALS/Animations/als_notify_manifest.json";

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

#[derive(Clone, Debug, PartialEq)]
pub struct AlsAnimationEventPlayback {
    pub instance_id: u64,
    pub interrupted: bool,
    pub source_asset: String,
    pub graph_clip: String,
    pub time: f32,
    pub previous_time: Option<f32>,
    pub weight: f32,
    pub looped: bool,
}

#[derive(Component, Clone, Debug, Default)]
pub struct AlsAnimationEventPlaybacks {
    pub entries: Vec<AlsAnimationEventPlayback>,
}

pub use crate::als::state::AlsNotifyLocomotionAction;

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
    instance_id: u64,
    montage_chance: NotifyChance,
}

impl AlsAnimationEventRuntime {
    fn set_instance(&mut self, instance_id: u64) {
        if self.instance_id != instance_id {
            self.instance_id = instance_id;
            self.montage_chance = NotifyChance::default();
        }
    }
}

struct NotifyChance(u32);

impl Default for NotifyChance {
    fn default() -> Self { Self(0x05629063) }
}

impl NotifyChance {
    fn accepts(&mut self, event: &AlsNotifyManifestEvent) -> bool {
        if event_kind(event) == TimedEventKind::State { return true; }
        self.0 = self.0.wrapping_mul(196314165).wrapping_add(907633515);
        f32::from_bits(0x3f800000 | (self.0 >> 9)) - 1.0 < event.filter.trigger_chance
    }
}

fn notify_root_instance(arena: &bevy_animation_graph::core::context::graph_context_arena::GraphContextArena) -> u64 {
    use bevy_animation_graph::core::{animation_graph::NodeId, context::node_states::StateKey};
    arena.get_toplevel().node_states
        .get::<crate::extensions::source_instance::SourceInstanceProperties>(NodeId::default(), StateKey::Default)
        .map_or(0, |state| state.instance_id)
}

#[derive(Default)]
struct GraphFootstepDispatch {
    frame: Option<(u64, u64)>,
    previous: HashMap<String, (f32, f32)>,
    slots: std::collections::HashSet<String>,
    chance: HashMap<u64, NotifyChance>,
}

fn graph_footstep_clocks(clock: &crate::extensions::SourceRelevantAsset, previous_time: f32, is_leader: bool) -> Vec<EventClock> {
    let mut remaining = if clock.has_previous_position {
        clock.playback_delta_seconds
    } else { clock.position - previous_time };
    if !is_leader && clock.has_previous_position {
        remaining = clock.position - previous_time;
        if remaining * clock.playback_delta_seconds < 0.0 {
            remaining += clock.playback_delta_seconds.signum() * clock.duration;
        }
    }
    if clock.duration <= 0.0 || remaining == 0.0 { return Vec::new(); }
    let backward = remaining < 0.0;
    let direction = if backward { PlaybackDirection::Backward } else { PlaybackDirection::Forward };
    let sign = remaining.signum();
    let max_segments = if remaining < -clock.duration {
        1000
    } else if remaining > clock.duration {
        ((remaining / clock.duration) as u32).clamp(2, 1000)
    } else { 2 };
    let mut position = previous_time;
    let mut segments = Vec::with_capacity(max_segments as usize);
    for _ in 0..max_segments {
        let end = (position + remaining).clamp(0.0, clock.duration);
        if end != position {
            segments.push(EventClock {
                previous_time: position, current_time: end,
                duration: clock.duration, direction, wrapped: false,
            });
        }
        remaining -= end - position;
        if !clock.looping || remaining * sign <= 0.0 { break; }
        position = if backward { clock.duration } else { 0.0 };
    }
    segments
}

fn dispatch_graph_footsteps(
    source_inputs: Query<&crate::extensions::SourceAnimationInputs>,
    manifest_handle: Option<Res<AlsNotifyManifestHandle>>,
    manifests: Res<Assets<AlsNotifyManifest>>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    server: Res<AssetServer>,
    avatars: Query<(Entity, &crate::character_assembly::AlsCharacterAvatar, &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance)>,
    players: Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
    mut runtimes: Query<(&crate::character_assembly::AlsCharacterAssembly, &mut AlsAnimationEventRuntime)>,
    mut frames: Local<HashMap<Entity, GraphFootstepDispatch>>,
    mut events: MessageWriter<AlsAnimationEvent>,
    mut shakes: MessageWriter<AlsCameraShakeRequest>,
) {
    use bevy_animation_graph::core::{animation_graph_player::AnimationSource, context::node_states::StateKey};
    use crate::extensions::sync_group_phase::source_sync_players;
    let Some(manifest) = manifest_handle.as_ref().and_then(|handle| manifests.get(&handle.0)) else { return; };
    for (avatar_entity, avatar, instance) in &avatars {
        let Ok((assembly, mut runtime)) = runtimes.get_mut(avatar.character) else { continue; };
        if assembly.avatar != avatar_entity { continue; }
        let entity = instance.player_entity();
        let Ok(player) = players.get(entity) else { continue; };
        if player.is_paused() || player.get_error().is_some() { continue; }
        let AnimationSource::Graph(handle) = player.get_animation_source() else { continue; };
        let Some(graph) = graphs.get(handle) else { continue; };
        let Some(arena) = player.get_context_arena() else { continue; };
        let Some(node) = crate::animation_context_projection::root_pose_source_node(graph) else { continue; };
        let Some(context) = crate::extensions::animation_context::node_context_state(&arena.get_toplevel().node_states, StateKey::Default, node) else { continue; };
        let source_instance = notify_root_instance(arena);
        runtime.set_instance(source_instance);
        let state = frames.entry(entity).or_default();
        let frame = (source_instance, arena.frame_index());
        if state.frame.is_some_and(|previous| previous.0 != source_instance) { *state = default(); }
        if state.frame == Some(frame) { continue; }
        state.frame = Some(frame);
        let current_slots = context.asset_samples.iter().filter(|sample| sample.montage && !sample.slot_key.is_empty())
            .map(|sample| sample.slot_key.clone()).collect::<std::collections::HashSet<_>>();
        let mut samples = context.asset_samples.clone();
        for slot in state.slots.difference(&current_slots) {
            if let Some(context) = source_inputs.get(entity).ok().and_then(|inputs| inputs.slot_input_context(slot)) {
                crate::extensions::animation_context::merge_asset_samples(&mut samples, &context.asset_samples, 1.0);
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
            if !sample.clock.has_previous_position && !sample.clock.looping && sample.clock.position < previous_time { continue; }
            let is_leader = if sample.sync_group.is_empty() { true } else {
                source_sync_players(arena, &graphs, &sample.sync_group)
                    .is_some_and(|players| players.iter().any(|player|
                        Some(player.identity) == sample.sync_identity && player.ticked_as_leader))
            };
            let sprint = source.source_asset == SOURCE_SPRINT || source.source_asset == SOURCE_SPRINT_ACCELERATION;
            let definitions = timed_events(source).into_iter().filter(|definition| {
                source.events.iter().any(|event| event.event_id == definition.id
                    && (event.class_name == "AlsAnimNotify_FootstepEffects"
                        || (sprint && event.class_name == "AlsAnimNotify_CameraShake")))
                    || (sprint && source.sync_markers.iter().any(|marker|
                        marker_event_id(&source.source_asset, &marker.id) == definition.id))
            }).collect::<Vec<_>>();
            let weight = if sample.montage { sample.clock.weight.max(previous.map_or(0.0, |previous| previous.1)) } else { sample.clock.weight };
            let mut signals = Vec::new();
            for clock in graph_footstep_clocks(&sample.clock, previous_time, is_leader) {
                let mut crossed = EventLifecycle::default().update_with_context(
                    &definitions, &EventQueue::default(), Some(clock), weight,
                    EventDispatchContext { is_leader, is_dedicated_server: false },
                );
                crossed.sort_by_key(|signal| source.events.iter().position(|event| event.event_id == signal.id).unwrap_or(usize::MAX));
                signals.extend(crossed);
            }
            for signal in signals {
                let (class_name, name, parameters) = if let Some(event) = source.events.iter().find(|event| event.event_id == signal.id) {
                    let chance = if sample.montage { &mut runtime.montage_chance }
                        else { state.chance.entry(sample.notify_instance_id).or_default() };
                    if !chance.accepts(event) { continue; }
                    if event.class_name == "AlsAnimNotify_CameraShake" && signal.phase == EventPhase::Point {
                        shakes.write(AlsCameraShakeRequest::sprint(avatar.character,
                            parameter_f32(&event.parameters, "CameraShakeScale", 1.0)));
                    }
                    (event.class_name.clone(), event.name.clone(), event.parameters.clone())
                } else if let Some(marker) = source.sync_markers.iter().find(|marker|
                    marker_event_id(&source.source_asset, &marker.id) == signal.id) {
                    ("AuthoredSyncMarker".to_owned(), marker.name.clone(), Value::Null)
                } else {
                    continue;
                };
                events.write(AlsAnimationEvent {
                    character: avatar.character,
                    instance_id: if sample.montage { sample.identity.rsplit(':').next().and_then(|value| value.parse().ok()).unwrap_or(0) } else { source_instance },
                    source_asset: source.source_asset.clone(), graph_clip: path.clone(), event_id: signal.id,
                    class_name, name, phase: signal.phase,
                    time: sample.clock.position, weight: signal.weight, percentage: signal.percentage, parameters,
                });
            }
        }
        state.previous.retain(|identity, _| active.contains(identity));
        state.chance.retain(|instance, _| arena.iter_context_ids().any(|context| arena.get_context(context).unwrap().node_states
            .get::<crate::extensions::source_instance::SourceInstanceProperties>(crate::extensions::source_instance::source_notify_instance_node(), StateKey::Default)
            .is_ok_and(|state| state.instance_id == *instance)));
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
            .configure_sets(PostUpdate, AlsAnimationEventSet::GraphFootsteps
                .after(AnimationGraphSet::Final).before(crate::als_camera::AlsCameraSet))
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
        previous_time: None,
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
    samplers: Query<(Entity, &crate::roll_drive::AlsRollSlotSampler, Option<&crate::roll_drive::AlsRollTail>)>,
    mut characters: Query<
        (
            Entity,
            &mut AlsAnimationEventPlaybacks,
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
    for (character, mut playbacks, roll, roll_blend_out, mantle, get_up) in &mut characters {
        let mut entries = Vec::new();
        let current = samplers.iter().find(|(_, sampler, tail)| sampler.character == character && tail.is_none()).map(|(entity, _, _)| entity.to_bits()).unwrap_or(0);
        let mut roll_instances = samplers.iter().filter_map(|(entity, sampler, tail)| {
            (sampler.character == character).then_some(tail.map(|tail| (entity.to_bits(), tail.time, None, tail.weight(), true))) .flatten()
        }).collect::<Vec<_>>();
        if let Some(roll) = roll.filter(|state| state.active) {
            roll_instances.push((current, roll.time, Some(roll.previous_time), 1.0, roll_blend_out.is_some()));
        }
        for (instance_id, elapsed, previous, weight, interrupted) in roll_instances {
            let time = (elapsed * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
            for source in [SOURCE_ROLL_SEQUENCE, SOURCE_ROLL_MONTAGE] {
                if let Some(graph_clip) = graph_clip_for_source(manifest, source) {
                    push_playback(&mut entries, source, graph_clip, time, weight, false, interrupted);
                    if let Some(entry) = entries.last_mut() {
                        entry.instance_id = instance_id;
                        entry.previous_time = previous.map(|time| (time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS));
                    }
                }
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
    let observed = runtime.previous_time.replace(playback.time);
    let previous = if observed == Some(playback.time) {
        playback.time
    } else {
        playback.previous_time.or(observed).unwrap_or(playback.time)
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
    parameters: &Value, has_input: bool, locomotion: crate::als::state::LocomotionState,
    rotation: crate::als::state::RotationMode, stance: Stance,
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
            Option<&crate::character_assembly::AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    mut events: MessageWriter<AlsAnimationEvent>,
    mut shakes: MessageWriter<AlsCameraShakeRequest>,
    avatars: Query<(Entity, &crate::character_assembly::AlsCharacterAvatar, Option<&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>)>,
    players: Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
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
        assembly,
    ) in &mut characters
    {
        if let Some(arena) = avatars.iter().find(|(entity, avatar, _)| avatar.character == character
            && assembly.is_none_or(|assembly| assembly.avatar == *entity))
            .and_then(|(_, _, instance)| instance)
            .and_then(|instance| players.get(instance.player_entity()).ok())
            .and_then(|player| player.get_context_arena()) {
            runtime.set_instance(notify_root_instance(arena));
        }
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
            let superseded_roll = [SOURCE_ROLL_SEQUENCE, SOURCE_ROLL_MONTAGE].contains(&source_asset.as_str())
                && playbacks.entries.iter().any(|entry| entry.source_asset == source_asset && entry.instance_id != instance_id && !entry.interrupted);
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
                    if !superseded_roll && event.class_name == "AlsAnimNotifyState_SetLocomotionAction" {
                        let next_action =
                            notify_action(parameter_tag(&event.parameters, "LocomotionAction"));
                        if *action == next_action {
                            *action = AlsNotifyLocomotionAction::None;
                        }
                    }
                    if !superseded_roll && event.class_name == "AlsAnimNotifyState_SetRootMotionScale" {
                        let scale = parameter_f32(&event.parameters, "TranslationScale", 1.0);
                        if (root_scale.0 - scale).abs() <= 1e-8 {
                            root_scale.0 = 1.0;
                        }
                    }
                    if !superseded_roll && event.class_name == "AlsAnimNotifyState_EarlyBlendOut" {
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
                if class_name == "AlsAnimNotify_FootstepEffects" && avatars.iter().any(|(_, avatar, _)| avatar.character == character) {
                    continue;
                }
                if event.is_some_and(|event| !runtime.montage_chance.accepts(event)) { continue; }
                if class_name == "AlsAnimNotify_CameraShake" && signal.phase == EventPhase::Point {
                    shakes.write(AlsCameraShakeRequest::sprint(
                        character,
                        parameter_f32(&parameters, "CameraShakeScale", 1.0),
                    ));
                }
                if class_name == "AlsAnimNotify_SetGroundedEntryMode"
                    && signal.phase == EventPhase::Point
                {
                    if let Ok(mode) = AlsGroundedEntryMode::new(parameter_tag(&parameters, "GroundedEntryMode").unwrap_or_default()) {
                        commands.entity(character).insert(mode);
                    }
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
                        EventPhase::Begin => root_scale.0 = scale,
                        EventPhase::End if (root_scale.0 - scale).abs() <= 1e-8 => {
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
pub(crate) mod tests {
    use super::*;

    pub(crate) fn configure_root_scale_notifications(app: &mut App, actor: Entity, definitions: &[(f32, f32, f32)]) {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::event_track::{EventTrack, TrackItemValue};
        use bevy_animation_graph::core::edge_data::events::AnimationEvent;
        let mut manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../../../assets/ALS/Animations/als_notify_manifest.json")).unwrap();
        let template = manifest.clips.iter().flat_map(|clip| &clip.events)
            .find(|event| event.class_name == "AlsAnimNotifyState_SetRootMotionScale").unwrap().clone();
        let source = manifest.clips.iter_mut().find(|clip| clip.source_asset == SOURCE_ROLL_SEQUENCE).unwrap();
        source.events.clear();
        source.sync_markers.clear();
        let graph_clip = source.runtime_segments[0].graph_clip.clone();
        let mut clip = GraphClip::default();
        let mut track = EventTrack { name: "root_scale".to_owned(), events: Vec::new() };
        for (index, &(start, end, scale)) in definitions.iter().enumerate() {
            let mut event = template.clone();
            event.event_id = format!("root_scale_{index}");
            event.kind = "state".to_owned();
            event.start_time = start;
            event.end_time = end;
            event.trigger_time_offset = 0.0;
            event.filter.trigger_chance = 1.0;
            event.parameters = serde_json::json!({"TranslationScale": scale});
            track.add_item(TrackItemValue { event: AnimationEvent::StringId(event.event_id.clone()), start_time: start, end_time: end });
            source.events.push(event);
        }
        clip.event_tracks.insert(track.name.clone(), track);
        let clip_handle = app.world_mut().resource_mut::<Assets<GraphClip>>().add(clip);
        app.world_mut().resource_mut::<AlsNotifyGraphClipHandles>().0.insert(graph_clip.clone(), clip_handle);
        for clip in &mut manifest.clips {
            if clip.source_asset != SOURCE_ROLL_SEQUENCE { clip.events.clear(); clip.sync_markers.clear(); }
        }
        let handle = app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>().add(manifest);
        app.insert_resource(AlsNotifyManifestHandle(handle));
        let mut entries = Vec::new();
        push_playback(&mut entries, SOURCE_ROLL_SEQUENCE, &graph_clip, 0.0, 1.0, false, false);
        app.world_mut().entity_mut(actor).insert((AlsAnimationEventPlaybacks { entries }, AlsNotifyRootMotionScale::default()));
        app.world_mut().run_system_once(ensure_animation_event_components).unwrap();
        app.world_mut().get_mut::<AlsAnimationEventRuntime>(actor).unwrap().sources.clear();
    }

    #[test]
    fn root_motion_scale_active_preserves_source_overlap_and_external_writes() {
        use bevy::ecs::system::RunSystemOnce;
        for (outer, inner, inner_start, inner_end) in [(0.5_f32, 2.0_f32, 0.2, 0.5), (0.5, 0.5, 0.2, 0.5), (1.0, 2.0, 0.2, 0.5), (0.5, 2.0, 0.81, 0.9), (0.5, 0.0, 1.2, 1.3)] {
            for external in [None, Some(0.0), Some(2.0), Some(outer), Some(outer + 0.00001)] {
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<GraphClip>().init_asset::<AlsNotifyManifest>()
                    .init_resource::<AlsNotifyGraphClipHandles>().add_message::<AlsAnimationEvent>().add_message::<AlsCameraShakeRequest>();
                let actor = app.world_mut().spawn(AlsCharacter).id();
                let definitions = if inner_start > 1.0 { vec![(0.1, 0.8, outer)] }
                    else { vec![(0.1, 0.8, outer), (inner_start, inner_end, inner)] };
                configure_root_scale_notifications(&mut app, actor, &definitions);
                let mut expected = 1.0_f32;
                let mut previous = 0.0_f32;
                for time in [0.0, 0.11, 0.21, 0.35, 0.46, 0.55, 0.7, 0.8, 0.82, 0.91] {
                    if time == 0.35 && let Some(value) = external {
                        expected = value;
                        app.world_mut().get_mut::<AlsNotifyRootMotionScale>(actor).unwrap().0 = value;
                    }
                    for &(start, end, scale) in &definitions {
                        if previous < end && time >= end && (expected - scale).abs() <= 1e-8 { expected = 1.0; }
                        if previous < start && time >= start { expected = scale; }
                    }
                    app.world_mut().get_mut::<AlsAnimationEventPlaybacks>(actor).unwrap().entries[0].time = time;
                    app.world_mut().run_system_once(dispatch_animation_events).unwrap();
                    assert!((app.world().get::<AlsNotifyRootMotionScale>(actor).unwrap().0 - expected).abs() < 1e-8,
                        "Wrong source lifecycle: outer={outer}, inner={inner}, external={external:?}, time={time}, expected={expected}");
                    previous = time;
                }
            }
        }
    }

    #[test]
    fn root_motion_scale_instance_exit_preserves_unmatched_external_values() {
        use bevy::ecs::system::RunSystemOnce;
        for external in [0.0_f32, 2.0, 2.00001, 0.5, 0.50001] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<GraphClip>().init_asset::<AlsNotifyManifest>()
                .init_resource::<AlsNotifyGraphClipHandles>().add_message::<AlsAnimationEvent>().add_message::<AlsCameraShakeRequest>();
            let actor = app.world_mut().spawn(AlsCharacter).id();
            configure_root_scale_notifications(&mut app, actor, &[(0.1, 0.8, 0.5), (0.2, 0.5, 2.0)]);
            app.world_mut().get_mut::<AlsAnimationEventPlaybacks>(actor).unwrap().entries[0].time = 0.3;
            app.world_mut().run_system_once(dispatch_animation_events).unwrap();
            app.world_mut().get_mut::<AlsNotifyRootMotionScale>(actor).unwrap().0 = external;
            app.world_mut().get_mut::<AlsAnimationEventPlaybacks>(actor).unwrap().entries.clear();
            app.world_mut().run_system_once(dispatch_animation_events).unwrap();
            let expected = if external == 0.5 || external == 2.0 { 1.0 } else { external };
            assert_eq!(app.world().get::<AlsNotifyRootMotionScale>(actor).unwrap().0, expected);
        }
    }

    #[test]
    fn point_notify_chance_matches_native_queue_and_preserves_state_notifies() {
        use bevy::ecs::system::RunSystemOnce;
        let mut manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../../../assets/ALS/Animations/als_notify_manifest.json")).unwrap();
        let source = manifest.clips.iter_mut().find(|source| source.source_asset == SOURCE_SPRINT).unwrap();
        let template = source.events.iter().find(|event| event.class_name == "AlsAnimNotify_CameraShake").unwrap().clone();
        let chances = [0.0, 1.0, 0.25, 0.5, 0.75, 0.0, 1.0, 0.5, 0.5];
        let accepted = [false, true, true, false, true, false, true, true, false];
        let seeds = [1546474474_i32, 1700686557, 424048428, -18432121, -1358584226, -1178120991, -598519296, 530668907, -511842670];
        source.events = chances.into_iter().enumerate().map(|(index, chance)| {
            let mut event = template.clone();
            event.event_id = format!("chance_{index}");
            event.start_time = (index + 1) as f32 * 0.1;
            event.end_time = event.start_time;
            event.trigger_time_offset = 0.0;
            event.filter.trigger_chance = chance;
            event
        }).collect();
        let mut filtered = template.clone();
        filtered.event_id = "filtered".into();
        filtered.start_time = 0.025;
        filtered.weight_threshold = 2.0;
        source.events.push(filtered);
        let mut state = template;
        state.event_id = "state".into();
        state.kind = "state".into();
        state.class_name = "AlsAnimNotifyState_EarlyBlendOut".into();
        state.start_time = 0.0;
        state.end_time = 2.0;
        state.filter.trigger_chance = 0.0;
        state.parameters = serde_json::json!({"BlendOutDuration": 2.0});
        source.events.push(state);
        source.sync_markers.truncate(1);
        source.sync_markers[0].time = 0.05;
        let graph_clip = source.runtime_segments[0].graph_clip.clone();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<GraphClip>().init_asset::<AlsNotifyManifest>()
            .init_resource::<AlsNotifyGraphClipHandles>()
            .add_message::<AlsAnimationEvent>().add_message::<AlsCameraShakeRequest>();
        let handle = app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>().add(manifest);
        app.insert_resource(AlsNotifyManifestHandle(handle));
        let actors = [0, 1].map(|_| {
            let mut entries = Vec::new();
            push_playback(&mut entries, SOURCE_SPRINT, &graph_clip, 0.0, 1.0, false, false);
            app.world_mut().spawn((AlsCharacter, AlsAnimationEventPlaybacks { entries })).id()
        });
        let world = app.world_mut();
        world.run_system_once(ensure_animation_event_components).unwrap();
        world.run_system_once(dispatch_animation_events).unwrap();
        world.resource_mut::<Messages<AlsAnimationEvent>>().clear();
        for index in 0..chances.len() {
            for actor in actors {
                world.get_mut::<AlsAnimationEventPlaybacks>(actor).unwrap().entries[0].time = (index + 1) as f32 * 0.1 + 0.01;
            }
            world.run_system_once(dispatch_animation_events).unwrap();
            let events = world.resource_mut::<Messages<AlsAnimationEvent>>().drain().collect::<Vec<_>>();
            assert_eq!(events.iter().filter(|event| event.phase == EventPhase::Point).count(), usize::from(accepted[index]) * 2);
            assert_eq!(world.resource_mut::<Messages<AlsCameraShakeRequest>>().drain().count(), usize::from(accepted[index]) * 2);
            for actor in actors {
                assert_eq!(world.get::<AlsAnimationEventRuntime>(actor).unwrap().montage_chance.0 as i32, seeds[index]);
                assert!(world.get::<AlsNotifyEarlyBlendOut>(actor).unwrap().active);
                assert_eq!(world.get::<AlsNotifyEarlyBlendOut>(actor).unwrap().blend_out_duration, 2.0);
            }
            if index == 0 { assert_eq!(events.iter().filter(|event| event.phase == EventPhase::Marker).count(), 2); }
        }
        let mut runtime = AlsAnimationEventRuntime::default();
        runtime.set_instance(1);
        let mut event = world.resource::<Assets<AlsNotifyManifest>>().get(&world.resource::<AlsNotifyManifestHandle>().0).unwrap().clips
            .iter().find(|source| source.source_asset == SOURCE_SPRINT).unwrap().events[3].clone();
        event.filter.trigger_chance = 0.5;
        for _ in 0..3 { assert!(runtime.montage_chance.accepts(&event)); }
        runtime.set_instance(1);
        assert!(!runtime.montage_chance.accepts(&event));
        runtime.set_instance(2);
        assert!(runtime.montage_chance.accepts(&event));
        assert_eq!(runtime.montage_chance.0 as i32, seeds[0]);
    }

    #[test]
    fn roll_first_interval_dispatches_the_actual_camera_shake() {
        use bevy::ecs::system::RunSystemOnce;
        for first_dt in [0.0_f32, 1.0/60.0, 0.05, 0.1, 0.2] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(first_dt)));
            app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().clear();
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = true;
            app.update();
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = false;
            assert!((app.world().get::<AlsRollState>(character).unwrap().time - first_dt).abs() < 1e-6);
            let count = app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().filter(|request| request.target == character).count();
            assert_eq!(count, usize::from(first_dt * ROLL_PLAY_RATE >= 0.1), "First interval dropped the .1 camera shake: dt={first_dt}");
            app.world_mut().run_system_once(dispatch_animation_events).unwrap();
            assert_eq!(app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().count(), 0, "Repeated same-frame dispatch duplicated a point");
            let mut total = count;
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
            for _ in 0..5 {
                app.update();
                total += app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().filter(|request| request.target == character).count();
            }
            assert_eq!(total, 2, "Default roll shake intervals were lost or duplicated");
            app.world_mut().get_mut::<AlsRollState>(character).unwrap().time = 0.0;
            app.update();
            assert_eq!(app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().filter(|request| request.target == character).count(), 1, "A seek used the last observed time instead of the true pre-step position");
            for _ in 0..13 { app.update(); }
            assert!(!app.world().get::<AlsRollState>(character).unwrap().active);
            app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().clear();
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = true;
            app.update();
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = false;
            assert_eq!(app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().filter(|request| request.target == character).count(), 1, "A replay inherited the old instance cursor");
        }
        let mut runtime = AlsAnimationEventSourceRuntime::default();
        let mut entries = Vec::new();
        push_playback(&mut entries, SOURCE_ROLL_SEQUENCE, "fixture", 0.6, 1.0, false, false);
        let mut playback = entries.remove(0);
        assert_eq!(playback_clock(&mut runtime, &playback).unwrap().previous_time, 0.6);
        runtime = default();
        playback.previous_time = Some(0.5);
        assert_eq!(playback_clock(&mut runtime, &playback).unwrap().previous_time, 0.5);
        runtime.previous_time = Some(0.1);
        playback.previous_time = Some(0.55);
        assert_eq!(playback_clock(&mut runtime, &playback).unwrap().previous_time, 0.55);
        assert_eq!(playback_clock(&mut runtime, &playback).unwrap().previous_time, 0.6);
    }

    #[test]
    fn roll_first_interval_late_assets_use_the_current_instance_interval() {
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let handle = app.world_mut().remove_resource::<AlsNotifyManifestHandle>().unwrap();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = false;
        for _ in 0..2 { app.update(); }
        app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().clear();
        app.insert_resource(handle);
        app.update();
        assert_eq!(app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().filter(|request| request.target == character).count(), 1, "Late notify data replayed the old .1 point or lost the current .466667 point");
    }

    #[test]
    fn grounded_entry_point_notify_assigns_empty_and_custom_tags() {
        use bevy::ecs::system::RunSystemOnce;
        let mut manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../../../assets/ALS/Animations/als_notify_manifest.json")).unwrap();
        let source = manifest.clips.iter_mut().find(|source| source.source_asset == SOURCE_ROLL_SEQUENCE).unwrap();
        let mut event = source.events.iter().find(|event| event.class_name == "AlsAnimNotify_SetGroundedEntryMode").unwrap().clone();
        event.event_id = "grounded_entry_empty".into();
        event.start_time = 0.1;
        event.end_time = 0.1;
        event.trigger_time_offset = 0.0;
        event.parameters = serde_json::json!({"GroundedEntryMode": {"__tagged__": {"TagName": ""}}});
        let tags = ["", "Als.GroundedEntryMode.Custom", "Als.GroundedEntryMode.FromRoll", "Als.GroundedEntryMode.FromRoll", ""];
        source.events = tags.iter().enumerate().map(|(index, tag)| {
            let mut event = event.clone();
            event.event_id = format!("grounded_entry_{index}");
            event.start_time = 0.1 + index as f32 * 0.2;
            event.end_time = event.start_time;
            event.parameters = serde_json::json!({"GroundedEntryMode": {"__tagged__": {"TagName": tag}}});
            event
        }).collect();
        source.sync_markers.clear();
        let graph_clip = source.runtime_segments[0].graph_clip.clone();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<GraphClip>().init_asset::<AlsNotifyManifest>()
            .init_resource::<AlsNotifyGraphClipHandles>().add_message::<AlsAnimationEvent>().add_message::<AlsCameraShakeRequest>();
        let handle = app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>().add(manifest);
        app.insert_resource(AlsNotifyManifestHandle(handle));
        let mut entries = Vec::new();
        push_playback(&mut entries, SOURCE_ROLL_SEQUENCE, &graph_clip, 0.0, 1.0, false, false);
        let character = app.world_mut().spawn((AlsCharacter, AlsAnimationEventPlaybacks { entries }, AlsGroundedEntryMode::FromRoll)).id();
        app.world_mut().run_system_once(ensure_animation_event_components).unwrap();
        app.world_mut().run_system_once(dispatch_animation_events).unwrap();
        for (index, tag) in tags.into_iter().enumerate() {
            app.world_mut().get_mut::<AlsAnimationEventPlaybacks>(character).unwrap().entries[0].time = 0.2 + index as f32 * 0.2;
            app.world_mut().run_system_once(dispatch_animation_events).unwrap();
            assert_eq!(app.world().get::<AlsGroundedEntryMode>(character).unwrap().as_str(), tag);
        }
    }

    #[test]
    fn custom_source_action_and_early_blend_tags_keep_their_identity() {
        assert_eq!(notify_action(Some("Als.LocomotionAction.Custom")).as_str(), "Als.LocomotionAction.Custom");
        let rotation = crate::als::state::RotationMode::new("Als.RotationMode.Custom").unwrap();
        let stance = Stance::new("Als.Stance.Custom").unwrap();
        let parameters = serde_json::json!({"bCheckInput": false, "bCheckLocomotionMode": false,
            "bCheckRotationMode": true, "RotationModeEquals": {"__tagged__": {"TagName": rotation.as_str()}}, "bCheckStance": false});
        assert!(early_blend_out_matches(&parameters, false, crate::als::state::LocomotionState::Grounded, rotation, stance));
        assert!(!early_blend_out_matches(&parameters, false, crate::als::state::LocomotionState::Grounded, crate::als::state::RotationMode::Aiming, stance));
    }

    #[test]
    fn graph_footstep_clocks_keep_source_loop_budget() {
        let clock = |previous, current, loops: u32, looping| graph_footstep_clocks(
            &crate::extensions::SourceRelevantAsset {
                duration: 1.0, position: current, playback_delta_seconds: loops as f32 + current - previous,
                has_previous_position: true, looping, ..default()
            }, previous, true);
        assert_eq!(clock(0.1, 0.2, 0, true).len(), 1);
        let wrapped = clock(0.9, 0.1, 1, true);
        assert_eq!(wrapped.len(), 2);
        assert_eq!((wrapped[0].previous_time, wrapped[0].current_time), (0.9, 1.0));
        assert_eq!(wrapped[1].previous_time, 0.0);
        assert!((wrapped[1].current_time - 0.1).abs() < 1e-6);
        assert_eq!(clock(0.1, 0.1, 2, true).len(), 2);
        assert_eq!(clock(0.1, 0.6, 2, true).len(), 2);
        assert_eq!(clock(0.1, 0.6, 2, false).len(), 1);
        assert!(graph_footstep_clocks(&crate::extensions::SourceRelevantAsset {
            duration: 1.0, position: 0.1, playback_delta_seconds: 1000.0, has_previous_position: true,
            looping: true, ..default()
        }, 0.1, true).len() <= 1000);
        assert!(graph_footstep_clocks(&crate::extensions::SourceRelevantAsset {
            duration: 1.0, playback_delta_seconds: -2000.0, has_previous_position: true, looping: true, ..default()
        }, 0.1, true).len() <= 1000);
        assert!(clock(0.1, 0.1, 0, true).is_empty());
        for (previous, current, delta) in [(0.9, 0.1, 2.5), (0.1, 0.9, -2.6)] {
            let follower = crate::extensions::SourceRelevantAsset {
                duration: 1.0, position: current, previous_position: previous, has_previous_position: true,
                playback_delta_seconds: delta, looping: true, ..default()
            };
            let segments = graph_footstep_clocks(&follower, previous, false);
            let actual = segments.iter().map(|clock| clock.current_time - clock.previous_time).sum::<f32>();
            assert!((actual - 0.2 * delta.signum()).abs() < 1e-6);
            assert!(graph_footstep_clocks(&crate::extensions::SourceRelevantAsset {
                position: previous, ..follower
            }, previous, false).is_empty());
        }
        assert!(graph_footstep_clocks(&crate::extensions::SourceRelevantAsset {
            playback_delta_seconds: 1.0, has_previous_position: true, ..default()
        }, 0.0, true).is_empty());
    }

    #[test]
    fn graph_notifications_ignore_retained_avatars_in_both_orders() {
        use bevy_animation_graph::core::{animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance},
            animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer,
            animation_node::AnimationNode, edge_data::DataSpec};
        use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};
        let (mut app, character, first, scene) = crate::camera_component::tests::loaded_camera_scene();
        app.world_mut().entity_mut(first)
            .remove::<(crate::camera_component::AlsSourceCameraComponent, crate::character_assembly::AlsCharacterCamera)>()
            .insert(AlsCharacterAvatar { character });
        app.world_mut().entity_mut(character).insert((AlsCharacter, AlsCharacterAssembly { avatar: first, camera: None }));
        let second = app.world_mut().spawn((AlsCharacterAvatar { character }, AnimatedSceneHandle::new(scene),
            Transform::from_xyz(10.0, 0.0, 0.0), ChildOf(character))).id();
        for _ in 0..20 {
            app.update();
            if app.world().get::<AnimatedSceneInstance>(second).is_some() { break; }
        }
        let players = [first, second].map(|avatar| app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity());
        let clip: Handle<GraphClip> = app.world().resource::<AssetServer>()
            .load("ALS/Animations/Grounded/Sprint/A_Als_Sprint.anim.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while app.world().resource::<Assets<GraphClip>>().get(&clip).is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Retained avatar source clip load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let skeleton = app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap().skeleton.clone();
        let node = AnimationNode::new("source sprint", crate::extensions::SourceSequencePlayerNode {
            clip, loop_animation: true, ..default()
        });
        let mut graph = AnimationGraph::default();
        graph.add_output_data("pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        graph.add_node(node);
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        for player in players {
            app.world_mut().entity_mut(player).remove::<crate::extensions::post_physics_animation::ManualAnimationPlayer>()
                .insert(AnimationGraphPlayer::new(skeleton.clone()).with_graph(graph.clone()));
        }
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        for selected in [0, 1] {
            let current = players[selected];
            let old = players[1 - selected];
            app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = [first, second][selected];
            app.world_mut().get_mut::<AnimationGraphPlayer>(current).unwrap().resume();
            app.world_mut().get_mut::<AnimationGraphPlayer>(old).unwrap().resume();
            app.update();
            let generation = notify_root_instance(app.world().get::<AnimationGraphPlayer>(current).unwrap().get_context_arena().unwrap());
            app.world_mut().get_mut::<AnimationGraphPlayer>(current).unwrap().pause();
            app.world_mut().get_mut::<AlsAnimationEventRuntime>(character).unwrap().montage_chance.0 = 17;
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().clear();
            for _ in 0..90 {
                app.update();
                let old_player = app.world().get::<AnimationGraphPlayer>(old).unwrap();
                assert!(old_player.get_error().is_none(), "{:?}", old_player.get_error());
                assert!(!app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                    .any(|event| event.character == character && event.source_asset == SOURCE_SPRINT),
                    "The retained moving avatar must not dispatch notifications for the stopped current avatar");
                assert_eq!(app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().count(), 0);
                let runtime = app.world().get::<AlsAnimationEventRuntime>(character).unwrap();
                assert_eq!(runtime.instance_id, generation);
                assert_eq!(runtime.montage_chance.0, 17);
            }
            app.world_mut().get_mut::<AnimationGraphPlayer>(current).unwrap().resume();
            let mut steps = 0;
            for _ in 0..90 {
                app.update();
                for event in app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain() {
                    if event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects" {
                        assert_eq!(event.instance_id, generation);
                        steps += 1;
                    }
                }
            }
            assert!(steps >= 2, "The current avatar must still dispatch its own source footsteps");
        }
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
        app.world_mut().entity_mut(character).insert((AlsCharacter,
            crate::character_assembly::AlsCharacterAssembly { avatar, camera: None }));
        let clip: Handle<GraphClip> = app.world().resource::<AssetServer>()
            .load("ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.anim.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while app.world().resource::<Assets<GraphClip>>().get(&clip).is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Footstep source clip load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let loaded_clip = app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap();
        let skeleton = loaded_clip.skeleton.clone();
        let clip_duration = loaded_clip.duration();
        let node = AnimationNode::new("source walk", crate::extensions::SourceSequencePlayerNode {
            clip: clip.clone(), loop_animation: true, ..default()
        });
        let mut graph = AnimationGraph::default();
        graph.add_output_data("pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        graph.add_node(node);
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        let forward_graph = graph.clone();
        app.world_mut().entity_mut(player_entity)
            .remove::<crate::extensions::post_physics_animation::ManualAnimationPlayer>()
            .insert(AnimationGraphPlayer::new(skeleton.clone()).with_graph(graph.clone()));
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
        let context = crate::extensions::animation_context::node_context_state(&arena.get_toplevel().node_states,
            bevy_animation_graph::core::context::node_states::StateKey::Default, node).unwrap();
        assert!((context.asset_samples[0].clock.playback_delta_seconds - clip_duration * 2.0).abs() < 1e-5);
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
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        for looping in [false, true] {
            let node = AnimationNode::new("reverse source walk", crate::extensions::SourceSequencePlayerNode {
                clip: clip.clone(), start_position: clip_duration * 0.9, play_rate: -1.0,
                loop_animation: looping, ..default()
            });
            let mut reverse = AnimationGraph::new();
            reverse.add_output_data("pose".to_owned(), DataSpec::Pose);
            reverse.add_output_data_edge(node.id, "Pose", "pose");
            reverse.add_node(node);
            let reverse = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(reverse);
            app.world_mut().entity_mut(player_entity).insert(AnimationGraphPlayer::new(skeleton.clone()).with_graph(reverse));
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            let mut reverse_feet = [0; 2];
            for _ in 0..150 {
                app.update();
                let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
                assert!(player.get_error().is_none(), "{:?}", player.get_error());
                for event in app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain() {
                    if event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects" {
                        let right = event.parameters.get("FootBone").and_then(|value| value.get("value"))
                            .and_then(|value| value.as_str()).is_some_and(|value| value.ends_with("::Right"));
                        reverse_feet[usize::from(right)] += 1;
                    }
                }
            }
            if looping { assert!(reverse_feet.iter().all(|count| *count >= 2), "Reverse looping footsteps: {reverse_feet:?}"); }
            else { assert_eq!(reverse_feet, [1, 1], "Reverse non-looping footsteps"); }
        }
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../../../assets/ALS/Animations/als_notify_manifest.json")).unwrap();
        let source = manifest.clips.iter().find(|source| source.source_asset.ends_with("/A_Als_Walk_Forward.A_Als_Walk_Forward")).unwrap();
        let native_cases: &[(bool, f32, &[usize])] = &[
            (false, 0.2, &[1]), (false, -0.2, &[]), (false, 1.2, &[1]), (false, -1.2, &[0]),
            (false, 2.6, &[1]), (false, -2.6, &[0]), (true, 0.2, &[1]), (true, -0.2, &[]),
            (true, 1.2, &[1, 0, 1]), (true, -1.2, &[0, 1]),
            (true, 2.6, &[1, 0, 1]), (true, -2.6, &[0, 0, 1, 0, 1]),
        ];
        for &(looping, periods, expected) in native_cases {
            let node = AnimationNode::new("native notify clock", crate::extensions::SourceSequencePlayerNode {
                clip: clip.clone(), start_position: clip_duration * 0.6, play_rate: periods.signum(),
                loop_animation: looping, ..default()
            });
            let mut graph = AnimationGraph::new();
            graph.add_output_data("pose".to_owned(), DataSpec::Pose);
            graph.add_output_data_edge(node.id, "Pose", "pose");
            graph.add_node(node);
            let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
            app.world_mut().entity_mut(player_entity).insert(AnimationGraphPlayer::new(skeleton.clone()).with_graph(graph));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
            app.update();
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f32(periods.abs() * clip_duration)));
            app.update();
            let actual = app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                .filter(|event| event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects")
                .map(|event| source.events.iter().position(|entry| entry.event_id == event.event_id).unwrap()).collect::<Vec<_>>();
            assert_eq!(actual, expected, "Native notify contract: looping={looping}, periods={periods}");
        }
        let manifest_handle = app.world().resource::<AlsNotifyManifestHandle>().0.clone();
        for (chance, expected) in [(0.0, 0), (0.5, 3), (0.5, 3), (1.0, 5)] {
            for source in &mut app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>().get_mut(&manifest_handle).unwrap().clips {
                for event in &mut source.events {
                    if event.class_name == "AlsAnimNotify_FootstepEffects" { event.filter.trigger_chance = chance; }
                }
            }
            app.world_mut().entity_mut(player_entity).insert(AnimationGraphPlayer::new(skeleton.clone()).with_graph(forward_graph.clone()));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            let mut footsteps = 0;
            for _ in 0..150 {
                app.update();
                footsteps += app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                    .filter(|event| event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects").count();
            }
            assert_eq!(footsteps, expected, "Point notify chance {chance} across frames and rebuilt players");
        }
        let mut linked = AnimationGraph::new();
        let blend = AnimationNode::new("linked instances", crate::extensions::SourceMultiWayBlendNode {
            pose_count: 2, desired_alpha_keys: vec![String::new(); 2],
            desired_alpha_defaults: vec![0.5, 0.5], ..default()
        });
        for index in 0..2 {
            let node = AnimationNode::new(format!("instance {index}"), crate::extensions::LayeredGraphNode {
                capture_context: true, internalize_time: true,
                ..crate::extensions::LayeredGraphNode::new(forward_graph.clone())
            });
            linked.add_node_parameter_edge(node.id, "pose", blend.id, format!("Poses {index}"));
            linked.add_node(node);
        }
        linked.add_output_data("pose".to_owned(), DataSpec::Pose);
        linked.add_output_data_edge(blend.id, "Pose", "pose");
        linked.add_node(blend);
        let linked = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(linked);
        for source in &mut app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>().get_mut(&manifest_handle).unwrap().clips {
            for event in &mut source.events { event.filter.trigger_chance = 0.5; }
        }
        app.world_mut().entity_mut(player_entity).insert(AnimationGraphPlayer::new(skeleton).with_graph(linked));
        let mut footsteps = 0;
        for _ in 0..150 {
            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none(), "{:?}", player.get_error());
            footsteps += app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                .filter(|event| event.character == character && event.class_name == "AlsAnimNotify_FootstepEffects").count();
        }
        assert_eq!(footsteps, 6, "Linked instances must have independent notify queues");
    }

    #[test]
    fn sprint_shake_uses_evaluated_sync_leader_and_clip_clock() {
        use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance,
            animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer,
            animation_node::AnimationNode, edge_data::DataSpec};
        let (mut app, character, avatar, _) = crate::camera_component::tests::loaded_camera_scene();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().entity_mut(avatar)
            .remove::<(crate::camera_component::AlsSourceCameraComponent, crate::character_assembly::AlsCharacterCamera)>()
            .insert(crate::character_assembly::AlsCharacterAvatar { character });
        app.world_mut().entity_mut(character).insert((AlsCharacter,
            crate::character_assembly::AlsCharacterAssembly { avatar, camera: None }));
        let clips = [SOURCE_SPRINT, SOURCE_SPRINT_ACCELERATION].map(|source| {
            let path = if source == SOURCE_SPRINT {
                "ALS/Animations/Grounded/Sprint/A_Als_Sprint.anim.ron"
            } else {
                "ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.anim.ron"
            };
            app.world().resource::<AssetServer>().load::<GraphClip>(path)
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while clips.iter().any(|clip| app.world().resource::<Assets<GraphClip>>().get(clip).is_none()) {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Sprint source clips load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let skeleton = app.world().resource::<Assets<GraphClip>>().get(&clips[0]).unwrap().skeleton.clone();
        let sequences = clips.into_iter().map(|clip| AnimationNode::new("sprint", crate::extensions::SourceSequencePlayerNode {
            clip,
            synchronization: Some(crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                key: "sprint.test".to_owned(),
                delta_seconds_key: "animation.delta_seconds".to_owned(),
                ..default()
            }),
            ..default()
        })).collect::<Vec<_>>();
        let blend = AnimationNode::new("blend", crate::extensions::SourceMultiWayBlendNode {
            pose_count: 2,
            desired_alpha_keys: vec!["sprint_base_weight".into(), "sprint_accel_weight".into()],
            desired_alpha_defaults: vec![0.5, 0.5],
            ..default()
        });
        let root = AnimationNode::new("root", crate::extensions::SourcePosePassthroughNode {
            output_time: false,
            synchronize_groups: true,
            ..default()
        });
        let mut graph = AnimationGraph::default();
        graph.add_output_data("pose".to_owned(), DataSpec::Pose);
        for (index, sequence) in sequences.iter().enumerate() {
            graph.add_node(sequence.clone());
            graph.add_node_parameter_edge(sequence.id, "Pose", blend.id, format!("Poses {index}"));
        }
        graph.add_node(blend.clone());
        graph.add_node(root.clone());
        graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
        graph.add_output_data_edge(root.id, "Pose", "pose");
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        app.world_mut().entity_mut(player_entity)
            .remove::<crate::extensions::post_physics_animation::ManualAnimationPlayer>()
            .insert(AnimationGraphPlayer::new(skeleton.clone()).with_graph(graph.clone()));
        {
            let mut player = app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap();
            player.set_global_input_data("animation.delta_seconds", 0.05_f32);
            player.set_global_input_data("sprint_base_weight", 0.5_f32);
            player.set_global_input_data("sprint_accel_weight", 0.5_f32);
        }
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
        app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().clear();
        app.update();
        assert!(app.world().get::<AnimationGraphPlayer>(player_entity).unwrap().get_error().is_none());
        assert_eq!(app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
            .filter(|event| event.class_name == "AlsAnimNotify_CameraShake").count(), 0);
        app.update();
        let events = app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain().collect::<Vec<_>>();
        assert_eq!(events.iter().filter(|event| event.class_name == "AlsAnimNotify_CameraShake").count(), 0);
        assert_eq!(events.iter().filter(|event| event.class_name == "AuthoredSyncMarker").count(), 1);
        app.update();
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert!(player.get_error().is_none(), "{:?}", player.get_error());
        assert_eq!(app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
            .filter(|event| event.class_name == "AlsAnimNotify_CameraShake").count(), 1);
        assert_eq!(app.world_mut().resource_mut::<Messages<AlsCameraShakeRequest>>().drain().count(), 1);

        use crate::extensions::sync_group_phase::{SourceSyncGroupRole, SOURCE_SYNC_INERTIAL_SCOPE};
        let manifest_handle = app.world().resource::<AlsNotifyManifestHandle>().0.clone();
        for source in &mut app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>().get_mut(&manifest_handle).unwrap().clips {
            for event in &mut source.events {
                event.filter.trigger_on_follower = false;
                event.filter.trigger_chance = 1.0;
                event.weight_threshold = 0.01;
            }
        }
        for role in [SourceSyncGroupRole::TransitionLeader, SourceSyncGroupRole::TransitionFollower] {
            let mut sequence = sequences[1].try_inner_downcast_ref::<crate::extensions::SourceSequencePlayerNode>().unwrap().clone();
            sequence.synchronization.as_mut().unwrap().role = role;
            let mut transition = AnimationNode::new("transition sprint", sequence);
            transition.id = sequences[1].id;
            app.world_mut().resource_mut::<Assets<AnimationGraph>>().get_mut(&graph).unwrap().add_node(transition);
            for (full, inertial) in [(false, false), (true, false), (true, true)] {
                let mut player = AnimationGraphPlayer::new(skeleton.clone()).with_graph(graph.clone());
                player.set_global_input_data("animation.delta_seconds", 0.05_f32);
                player.set_global_input_data("sprint_base_weight", if full { 0.0_f32 } else { 0.4_f32 });
                player.set_global_input_data("sprint_accel_weight", if full { 1.0_f32 } else { 0.6_f32 });
                app.world_mut().entity_mut(player_entity).insert(player);
                app.update();
                {
                    let mut player = app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap();
                    player.set_global_input_data("sprint_base_weight", 0.4_f32);
                    player.set_global_input_data("sprint_accel_weight", 0.6_f32);
                    player.set_global_input_data(SOURCE_SYNC_INERTIAL_SCOPE, inertial);
                }
                app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
                let mut footsteps = [0; 2];
                for _ in 0..60 {
                    app.update();
                    let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
                    assert!(player.get_error().is_none(), "{:?}", player.get_error());
                    for event in app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                        .filter(|event| event.class_name == "AlsAnimNotify_FootstepEffects") {
                        if let Some(index) = [SOURCE_SPRINT, SOURCE_SPRINT_ACCELERATION].iter()
                            .position(|source| *source == event.source_asset) { footsteps[index] += 1; }
                    }
                }
                let independent = !full || inertial;
                let transition_leads = role == SourceSyncGroupRole::TransitionLeader;
                assert_eq!(footsteps.map(|count| count > 0),
                    [independent || !transition_leads, independent || transition_leads],
                    "role={role:?} full={full} inertial={inertial}: {footsteps:?}");
            }
        }

        use crate::extensions::source_sequence_player::{SourceSequenceSynchronization, SourceSequencePlayerState};
        use crate::extensions::sync_group_phase::{SyncMarker, source_sync_leader};
        use bevy_animation_graph::core::context::node_states::StateKey;
        for (start, delta, rate, looped, marked, expected_leader, expected_times) in [
            ([0.625, 1.25], 0.25, 1.0, false, true, 1, [0.875, 1.5]),
            ([0.375, 0.75], 0.25, -1.0, false, true, 1, [0.125, 0.5]),
            ([0.5, 1.0], 0.25, 1.0, false, true, 0, [0.75, 1.5]),
            ([0.5, 1.0], 0.25, -1.0, false, true, 0, [0.25, 0.5]),
            ([0.625, 1.25], 0.6, 1.0, false, true, 1, [1.0, 1.85]),
            ([0.625, 1.25], 0.25, 1.0, true, true, 0, [0.875, 1.75]),
            ([0.625, 1.25], 0.25, 1.0, false, false, 0, [0.875, 1.75]),
        ] {
            for (index, sequence) in sequences.iter().enumerate() {
                let duration = (index + 1) as f32;
                let mut node = sequence.try_inner_downcast_ref::<crate::extensions::SourceSequencePlayerNode>().unwrap().clone();
                app.world_mut().resource_mut::<Assets<GraphClip>>().get_mut(&node.clip).unwrap().duration = duration;
                node.start_position = start[index];
                node.play_rate = rate;
                node.loop_animation = looped;
                let phase_markers = if marked { vec![SyncMarker { name: "A".into(), time: 0.25 }, SyncMarker { name: "B".into(), time: 0.75 }] } else { vec![] };
                node.synchronization = Some(SourceSequenceSynchronization {
                    key: "boundary.test".into(), delta_seconds_key: "animation.delta_seconds".into(), phase_duration: 1.0,
                    markers: phase_markers.iter().map(|marker| SyncMarker { name: marker.name.clone(), time: marker.time * duration }).collect(),
                    phase_markers, role: if index == 0 { SourceSyncGroupRole::AlwaysLeader } else { SourceSyncGroupRole::AlwaysFollower }, ..default()
                });
                let mut replacement = AnimationNode::new("boundary player", node);
                replacement.id = sequence.id;
                app.world_mut().resource_mut::<Assets<AnimationGraph>>().get_mut(&graph).unwrap().add_node(replacement);
                let mut manifests = app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>();
                let mut manifest = manifests.get_mut(&manifest_handle).unwrap();
                let source = manifest.clips
                    .iter_mut().find(|clip| clip.source_asset == [SOURCE_SPRINT, SOURCE_SPRINT_ACCELERATION][index]).unwrap();
                let mut event = source.events.iter().find(|event| event.class_name == "AlsAnimNotify_FootstepEffects").unwrap().clone();
                event.start_time = start[index] + delta * rate * 0.5;
                event.end_time = event.start_time;
                event.trigger_time_offset = 0.0;
                source.events = vec![event];
                source.sync_markers.clear();
            }
            let mut player = AnimationGraphPlayer::new(skeleton.clone()).with_graph(graph.clone());
            player.set_global_input_data("animation.delta_seconds", 0.0_f32);
            player.set_global_input_data("sprint_base_weight", 0.5_f32);
            player.set_global_input_data("sprint_accel_weight", 0.5_f32);
            app.world_mut().entity_mut(player_entity).insert(player);
            app.update();
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().set_global_input_data("animation.delta_seconds", delta);
            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none(), "{:?}", player.get_error());
            let arena = player.get_context_arena().unwrap();
            let actual_leader = source_sync_leader(arena, app.world().resource::<Assets<AnimationGraph>>(), "boundary.test").unwrap();
            assert_eq!(actual_leader.identity.1, sequences[expected_leader].id, "start={start:?} delta={delta} rate={rate} looped={looped} marked={marked}");
            for (index, sequence) in sequences.iter().enumerate() {
                let state = arena.get_toplevel().node_states.get::<SourceSequencePlayerState>(sequence.id, StateKey::Default).unwrap();
                let actual = state.context.relevant_asset.as_ref().unwrap().position;
                assert!((actual - expected_times[index]).abs() < 1e-5, "player={index} start={start:?} delta={delta} rate={rate}: {actual} != {}", expected_times[index]);
            }
            let events = app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain().collect::<Vec<_>>();
            let counts = [SOURCE_SPRINT, SOURCE_SPRINT_ACCELERATION].map(|source| events.iter()
                .filter(|event| event.source_asset == source && event.class_name == "AlsAnimNotify_FootstepEffects").count());
            assert_eq!(counts, [1, usize::from(expected_leader == 1)], "start={start:?} delta={delta} rate={rate}: {counts:?}");
        }

        use bevy_animation_graph::builtin_nodes::clip_node::ClipNode;
        use bevy_animation_graph::core::animation_graph::{GraphInputPin, TimeUpdate};
        use crate::extensions::normalized_blendspace::{NormalizedBlendSpaceNode, NormalizedBlendSpacePoint, NormalizedBlendSpaceState};
        use crate::extensions::sync_group_phase::SourceSyncPlayerBinding;
        let clip = sequences[0].try_inner_downcast_ref::<crate::extensions::SourceSequencePlayerNode>().unwrap().clip.clone();
        for (resync, blendspace_leader, joins_as_follower, mode, sample_duration, group_duration) in [
            (false, true, false, "markers", 1.0, 1.0), (true, true, false, "markers", 1.0, 1.0), (true, false, false, "markers", 1.0, 1.0),
            (true, false, true, "markers", 1.0, 1.0), (true, true, true, "markers", 1.0, 1.0),
            (true, false, true, "markers", 2.0, 1.0), (true, true, true, "markers", 2.0, 1.0),
            (true, false, true, "length", 1.0, 1.0), (true, true, true, "length", 1.0, 1.0),
            (true, false, true, "length", 2.0, 1.0), (true, true, true, "length", 2.0, 1.0),
            (true, false, true, "length", 1.0, 2.0), (true, true, true, "length", 1.0, 2.0),
            (true, false, true, "mixed", 2.0, 1.0), (true, true, true, "mixed", 2.0, 1.0),
        ] {
            let markers = if mode == "length" { vec![] } else { vec![SyncMarker { name: "A".into(), time: 0.25 }, SyncMarker { name: "B".into(), time: 0.75 }] };
            let phase_markers = if mode == "mixed" { vec![SyncMarker { name: "A".into(), time: 0.125 }, SyncMarker { name: "B".into(), time: 0.625 }] } else { markers.clone() };
            let sample_markers = if mode == "mixed" { vec![] } else { markers.iter().map(|marker| SyncMarker { name: marker.name.clone(), time: marker.time * sample_duration }).collect() };
            app.world_mut().resource_mut::<Assets<GraphClip>>().get_mut(&clip).unwrap().duration = sample_duration;
            let mut follower = sequences[1].try_inner_downcast_ref::<crate::extensions::SourceSequencePlayerNode>().unwrap().clone();
            assert_ne!(clip, follower.clip);
            let follower_duration = if joins_as_follower { group_duration } else { 2.0 };
            app.world_mut().resource_mut::<Assets<GraphClip>>().get_mut(&follower.clip).unwrap().duration = follower_duration;
            follower.start_position = if joins_as_follower { 0.8125 * follower_duration } else if resync { 1.6 } else { 1.25 };
            follower.loop_animation = resync;
            follower.synchronization = Some(SourceSequenceSynchronization {
                key: "sample.history".into(), role: if joins_as_follower { SourceSyncGroupRole::AlwaysLeader } else { SourceSyncGroupRole::AlwaysFollower },
                delta_seconds_key: "animation.delta_seconds".into(), phase_duration: 1.0,
                phase_markers: phase_markers.clone(), markers: markers.iter().map(|marker|
                    SyncMarker { name: marker.name.clone(), time: marker.time * follower_duration }).collect(), ..default()
            });
            let follower = AnimationNode::new("history follower", follower);
            let sample = AnimationNode::new("history clip", ClipNode::new(clip.clone(), None, None));
            let role = if joins_as_follower { SourceSyncGroupRole::AlwaysFollower } else { SourceSyncGroupRole::AlwaysLeader };
            let start = if joins_as_follower { 0.25 } else if resync { 0.2 } else { 0.625 };
            let leader = if blendspace_leader { AnimationNode::new("history blendspace", NormalizedBlendSpaceNode {
                looped: resync, phase_duration: 1.0, phase_markers: phase_markers.clone(),
                points: vec![NormalizedBlendSpacePoint { id: "sample".into(), duration: sample_duration, markers: sample_markers.clone(), ..default() }],
                playback: Some(SourceSyncPlayerBinding { key: "sample.history".into(), role,
                    delta_seconds_key: "animation.delta_seconds".into(), start_position: start, ..default() }), ..default()
            })
            } else {
                AnimationNode::new("history sequence", crate::extensions::SourceSequencePlayerNode {
                    clip: clip.clone(), start_position: start * sample_duration, loop_animation: true,
                    synchronization: Some(SourceSequenceSynchronization { key: "sample.history".into(),
                        role, delta_seconds_key: "animation.delta_seconds".into(),
                        phase_duration: 1.0, phase_markers, markers: sample_markers, ..default() }), ..default()
                })
            };
            let mut history = AnimationGraph::default();
            history.add_output_data("pose".into(), DataSpec::Pose);
            if blendspace_leader {
                history.set_default_data(GraphInputPin::passthrough("position"), bevy_animation_graph::core::edge_data::DataValue::Vec2(Vec2::ZERO));
                history.add_input_data_edge(GraphInputPin::passthrough("position"), leader.id, crate::extensions::normalized_blendspace::IN_POSITION);
                history.add_node_parameter_edge(sample.id, ClipNode::OUT_POSE, leader.id, NormalizedBlendSpaceNode::pose_pin("sample"));
                history.add_node_pose_edge(sample.id, leader.id, NormalizedBlendSpaceNode::time_pin("sample"));
                history.add_node(sample);
            }
            history.add_node_parameter_edge(leader.id, if blendspace_leader { crate::extensions::normalized_blendspace::OUT_POSE } else { "Pose" }, blend.id, "Poses 0");
            history.add_node_parameter_edge(follower.id, "Pose", blend.id, "Poses 1");
            history.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
            history.add_output_data_edge(root.id, "Pose", "pose");
            for node in [leader.clone(), follower.clone(), blend.clone(), root.clone()] { history.add_node(node); }
            let history = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(history);
            {
                let mut manifests = app.world_mut().resource_mut::<Assets<AlsNotifyManifest>>();
                let mut manifest = manifests.get_mut(&manifest_handle).unwrap();
                let source = manifest.clips.iter_mut().find(|clip| clip.source_asset == SOURCE_SPRINT).unwrap();
                let template = source.events[0].clone();
                let events = if joins_as_follower { [("stale_a", 0.5), ("current_b", 0.84375)] }
                    else { [("sample_a", if resync { 0.25 } else { 0.78125 }), ("sample_b", if resync { 0.85 } else { 0.90625 })] };
                source.events = events
                    .map(|(id, time)| { let mut event = template.clone(); event.event_id = id.into(); event.id = id.into();
                        event.start_time = time * sample_duration; event.end_time = event.start_time; event.trigger_time_offset = 0.0;
                        event.filter.trigger_on_follower = joins_as_follower; event.filter.trigger_chance = 1.0; event }).to_vec();
            }
            let mut player = AnimationGraphPlayer::new(skeleton.clone()).with_graph(history);
            player.set_global_input_data("animation.delta_seconds", 0.0_f32);
            player.set_global_input_data("sprint_base_weight", if resync { 0.0_f32 } else { 0.5_f32 });
            player.set_global_input_data("sprint_accel_weight", if resync { 1.0_f32 } else { 0.5_f32 });
            app.world_mut().entity_mut(player_entity).insert(player);
            app.update();
            let frames = if joins_as_follower { vec![(0.0625, 0.8125, 0.875, Some("current_b")), (0.0625, 0.875, 0.9375, None), (0.03125, 0.9375, 0.96875, None)] }
                else if resync { vec![(0.1, 0.8, 0.9, Some("sample_b"))] }
                else { vec![(0.25, 0.625, 0.875, Some("sample_a")), (0.0625, 0.875, 0.9375, Some("sample_b")), (0.03125, 0.9375, 0.96875, None)] };
            for (delta, previous, current, expected) in frames {
                let (previous, current) = (previous * sample_duration, current * sample_duration);
                let delta = delta * if joins_as_follower { group_duration } else { 1.0 };
                app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
                {
                    let mut player = app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap();
                    player.set_global_input_data("animation.delta_seconds", delta);
                    player.set_global_input_data("sprint_base_weight", 0.5_f32);
                    player.set_global_input_data("sprint_accel_weight", 0.5_f32);
                }
                app.update();
                let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
                assert!(player.get_error().is_none(), "{:?}", player.get_error());
                let arena = player.get_context_arena().unwrap();
                let context = if blendspace_leader {
                    &arena.get_toplevel().node_states.get::<NormalizedBlendSpaceState>(leader.id, StateKey::Default).unwrap().context
                } else { &arena.get_toplevel().node_states.get::<SourceSequencePlayerState>(leader.id, StateKey::Default).unwrap().context };
                assert_eq!(context.asset_samples.len(), 1);
                let clock = context.asset_samples[0].clock.clone();
                assert_eq!(context.asset_samples[0].clip, clip);
                assert!((clock.position - current).abs() < 1e-5);
                if blendspace_leader { assert!(matches!(arena.get_toplevel().node_caches.get_input_time_update(leader.id, StateKey::Default,
                    NormalizedBlendSpaceNode::time_pin("sample")).unwrap(), TimeUpdate::Absolute(time) if (time-current).abs()<1e-5)); }
                assert_eq!(source_sync_leader(arena, app.world().resource::<Assets<AnimationGraph>>(), "sample.history").unwrap().identity.1,
                    if resync && !joins_as_follower { leader.id } else { follower.id });
                if joins_as_follower {
                    let member = arena.get_toplevel().node_states.get::<crate::extensions::AnimationContextState>(root.id, StateKey::Default).unwrap()
                        .sync_players.iter().find(|member| member.identity.1 == leader.id).unwrap();
                    assert!(!member.ticked_as_leader && !member.selected_leader);
                    let selected = arena.get_toplevel().node_states.get::<SourceSequencePlayerState>(follower.id, StateKey::Default).unwrap();
                    assert!((selected.context.relevant_asset.as_ref().unwrap().position / follower_duration - current / sample_duration).abs() < 1e-5);
                }
                let events = app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                    .filter(|event| event.source_asset == SOURCE_SPRINT && event.class_name == "AlsAnimNotify_FootstepEffects")
                    .map(|event| event.event_id).collect::<Vec<_>>();
                assert_eq!(events, expected.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                    "blendspace={blendspace_leader} resync={resync} follower={joins_as_follower} mode={mode} duration={sample_duration} group_duration={group_duration} previous={} current={} expected_previous={previous}", clock.previous_position, clock.position);
                assert!((clock.previous_position - previous).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn synchronized_sequence_respects_paused_player_time() {
        use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance,
            animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer,
            animation_node::AnimationNode, edge_data::DataSpec};
        let (mut app, character, avatar, _) = crate::camera_component::tests::loaded_camera_scene();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().entity_mut(avatar)
            .remove::<(crate::camera_component::AlsSourceCameraComponent, crate::character_assembly::AlsCharacterCamera)>()
            .insert(crate::character_assembly::AlsCharacterAvatar { character });
        app.world_mut().entity_mut(character).insert((AlsCharacter,
            crate::character_assembly::AlsCharacterAssembly { avatar, camera: None }));
        let clip: Handle<GraphClip> = app.world().resource::<AssetServer>()
            .load("ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.anim.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while app.world().resource::<Assets<GraphClip>>().get(&clip).is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Synchronized source clip load timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let skeleton = app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap().skeleton.clone();
        let node = AnimationNode::new("synced walk", crate::extensions::SourceSequencePlayerNode {
            clip,
            synchronization: Some(crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                key: "sync.pause".to_owned(),
                delta_seconds_key: "animation.delta_seconds".to_owned(),
                ..default()
            }),
            ..default()
        });
        let mut graph = AnimationGraph::default();
        graph.add_output_data("pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        graph.add_node(node);
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        app.world_mut().entity_mut(player_entity)
            .remove::<crate::extensions::post_physics_animation::ManualAnimationPlayer>()
            .insert(AnimationGraphPlayer::new(skeleton).with_graph(graph));
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap()
            .set_global_input_data("animation.delta_seconds", 0.1_f32);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
        for _ in 0..3 { app.update(); }
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert!(player.get_error().is_none(), "{:?}", player.get_error());
        let before = player.get_default_output_pose().unwrap().timestamp;
        assert!(before > 0.0);
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().pause();
        app.update();
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert!(player.get_error().is_none(), "{:?}", player.get_error());
        assert!(player.get_global_input_data("animation.delta_seconds").unwrap().as_f32().unwrap() > 0.0);
        let after = player.get_default_output_pose().unwrap().timestamp;
        assert!((after - before).abs() < 1e-5, "Paused sync clock advanced from {before} to {after}");
    }

    #[test]
    fn source_zero_frame_footsteps_keep_their_trigger_offset() {
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../../../assets/ALS/Animations/als_notify_manifest.json")).unwrap();
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
                "../../../assets/ALS/Animations/als_notify_manifest.json"
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
        use crate::als::state::{LocomotionState, RotationMode};
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
            "../../../assets/ALS/Animations/als_notify_manifest.json"
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
            "../../../assets/ALS/Animations/als_notify_manifest.json"
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
        let manifest: AlsNotifyManifest = serde_json::from_str(include_str!("../../../assets/ALS/Animations/als_notify_manifest.json")).unwrap();
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
