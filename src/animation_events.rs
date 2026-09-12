use std::collections::HashMap;

use als_camera::AlsCameraShakeRequest;
use als_core::character::AlsCharacter;
use als_core::mantle::{AlsMantleState, MantleKind, mantle_weight_at};
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

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub enum AlsNotifyLocomotionAction {
    #[default]
    None,
    Mantling,
    GettingUp,
    Rolling,
}

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
    sources: HashMap<String, AlsAnimationEventSourceRuntime>,
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
                    .before(AlsMantleDriveSet),
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
) {
    if !time.is_finite() || !weight.is_finite() || weight <= f32::EPSILON {
        return;
    }
    entries.push(AlsAnimationEventPlayback {
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
            Option<&AlsMantleState>,
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
    for (mut playbacks, standing, roll, mantle, get_up) in &mut characters {
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
                );
            }
        }
        if let Some(mantle) = mantle.filter(|state| state.active) {
            let (sequence, montage) = mantle_sources(mantle.kind);
            let time = mantle.clip_time_at(mantle.time);
            let weight = mantle_weight_at(mantle.time, mantle.action_end());
            if let Some(graph_clip) = graph_clip_for_source(manifest, sequence) {
                push_playback(&mut entries, sequence, graph_clip, time, weight, false);
            }
            if let Some(graph_clip) = graph_clip_for_source(manifest, montage) {
                push_playback(&mut entries, montage, graph_clip, time, weight, false);
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
            start_time: event.start_time,
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

fn parameter_f32(parameters: &Value, key: &str, default: f32) -> f32 {
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

fn notify_action(tag: Option<&str>) -> AlsNotifyLocomotionAction {
    match tag {
        Some("Als.LocomotionAction.Mantling") => AlsNotifyLocomotionAction::Mantling,
        Some("Als.LocomotionAction.GettingUp") => AlsNotifyLocomotionAction::GettingUp,
        Some("Als.LocomotionAction.Rolling") => AlsNotifyLocomotionAction::Rolling,
        _ => AlsNotifyLocomotionAction::None,
    }
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
        let mut active_sources = HashMap::<String, bool>::default();
        for playback in &playbacks.entries {
            let Some(source) = source_clip(manifest, &playback.source_asset) else {
                continue;
            };
            let graph_handle =
                graph_clip_handle(&mut graph_clip_handles, &asset_server, &playback.graph_clip);
            let queue = sampled_queue(graph_clips.get(&graph_handle), playback.time);
            let source_runtime = runtime
                .sources
                .entry(playback.source_asset.clone())
                .or_default();
            let clock = playback_clock(source_runtime, playback);
            let definitions = timed_events(source);
            let signals = source_runtime.lifecycle.update_with_context(
                &definitions,
                &queue,
                clock,
                playback.weight,
                EventDispatchContext::default(),
            );
            active_sources.insert(playback.source_asset.clone(), true);
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
                        EventPhase::Begin | EventPhase::Active => *action = next_action,
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
        let inactive = runtime
            .sources
            .keys()
            .filter(|source| !active_sources.contains_key(*source))
            .cloned()
            .collect::<Vec<_>>();
        for source_asset in inactive {
            let Some(source) = source_clip(manifest, &source_asset) else {
                continue;
            };
            let source_runtime = runtime
                .sources
                .get_mut(&source_asset)
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
            runtime.sources.remove(&source_asset);
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
