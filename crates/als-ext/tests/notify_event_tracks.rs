use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use als_ext::RootMotionClipNode;
use bevy::{
    asset::{Assets, Handle},
    ecs::system::{Res, ResMut, SystemParamItem},
    platform::collections::HashMap,
};
use bevy_animation_graph::builtin_nodes::clip_node::ClipNode;
use bevy_animation_graph::core::{
    animation_clip::{GraphClip, loader::GraphClipSerial},
    animation_graph::{AnimationGraph, GraphInputPin, SourcePin, TargetPin, TimeUpdate},
    animation_node::AnimationNode,
    context::spec_context::{NodeInput, NodeOutput, NodeSpec, SpecResources},
    edge_data::events::{AnimationEvent, EventQueue, SampledEvent},
    event_track::{EventTrack, TrackItem, TrackItemValue, sample_tracks},
    pose::RootMotionMode,
    skeleton::Skeleton,
    state_machine::high_level::StateMachine,
    utils::testing::{GraphTestHarness, SystemExtractor},
};

const ROLL: &str = "animations/Actions/Roll/A_Als_Roll.anim.ron";
const SPRINT: &str = "animations/Grounded/Sprint/A_Als_Sprint.anim.ron";

fn load(path: &str) -> GraphClipSerial {
    let path = assets_root().join(path);
    let text = fs::read_to_string(path).expect("graph clip must be readable");
    ron::de::from_str(&text).expect("graph clip event tracks must deserialize")
}

fn assets_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn string_id(event: &AnimationEvent) -> Option<&str> {
    match event {
        AnimationEvent::StringId(value) => Some(value),
        _ => None,
    }
}

fn assert_clip_queue(track: EventTrack, update: TimeUpdate, expected: EventQueue) {
    let handle = Handle::<GraphClip>::default();
    GraphTestHarness::new(ClipExtractor {
        node: AnimationNode::new("clip", ClipNode::new(handle.clone(), None, None)),
        handle,
        track,
    })
    .with_time_fwd(Some(update))
    .when_queried()
    .then_output_is(ClipNode::OUT_EVENT_QUEUE, expected);
}

struct ClipExtractor {
    node: AnimationNode,
    handle: Handle<GraphClip>,
    track: EventTrack,
}

impl SystemExtractor for ClipExtractor {
    type Param<'w, 's> = (
        Res<'w, Assets<AnimationGraph>>,
        Res<'w, Assets<StateMachine>>,
        ResMut<'w, Assets<GraphClip>>,
    );
    type Out = NodeSpec;

    fn process<'w, 's>(
        &self,
        (graphs, machines, mut clips): SystemParamItem<Self::Param<'w, 's>>,
    ) -> Self::Out {
        let mut tracks = HashMap::default();
        tracks.insert(self.track.name.clone(), self.track.clone());
        clips
            .insert(
                self.handle.id(),
                GraphClip {
                    curves: Default::default(),
                    source: None,
                    duration: 1.5,
                    skeleton: Handle::<Skeleton>::default(),
                    event_tracks: tracks,
                },
            )
            .expect("test graph clip must insert");
        let resources = SpecResources {
            graph_assets: &graphs,
            fsm_assets: &machines,
        };
        let mut spec = NodeSpec::default();
        self.node
            .new_spec(resources)
            .map(|value| spec = value)
            .expect("clip node spec must build");
        spec
    }

    fn graph_setup(&self, graph: &mut AnimationGraph, spec: Self::Out) {
        graph.add_node(self.node.clone());
        for input in spec.sorted_inputs() {
            match input {
                NodeInput::Time(pin) => {
                    graph.add_input_time(GraphInputPin::Passthrough(pin.clone()));
                    graph.add_input_time_edge(
                        GraphInputPin::Passthrough(pin.clone()),
                        self.node.id,
                        pin,
                    );
                }
                NodeInput::Data(pin, data_spec) => {
                    graph.add_input_data(GraphInputPin::Passthrough(pin.clone()), data_spec);
                    graph.add_input_data_edge(
                        GraphInputPin::Passthrough(pin.clone()),
                        self.node.id,
                        pin,
                    );
                }
            }
        }
        for output in spec.sorted_outputs() {
            match output {
                NodeOutput::Time => {
                    graph.add_output_time();
                    graph.add_edge(SourcePin::NodeTime(self.node.id), TargetPin::OutputTime);
                }
                NodeOutput::Data(pin, data_spec) => {
                    graph.add_output_data(pin.clone(), data_spec);
                    graph.add_output_data_edge(self.node.id, pin.clone(), pin);
                }
            }
        }
    }
}

#[test]
fn roll_state_track_samples_through_actual_graph_clip_serialization() {
    let clip = load(ROLL);
    let sampled = sample_tracks(clip.event_tracks.values(), 0.75);
    let event = sampled
        .iter()
        .find(|event| {
            string_id(&event.event)
                .is_some_and(|value| value.contains("decc527d-454b1467-02c76397-aa1bfb95"))
        })
        .expect("root motion state must be active at half roll time");
    assert!((event.percentage - 0.5).abs() < 1e-5);
}

#[test]
fn point_track_preserves_exact_source_timestamp() {
    let clip = load(ROLL);
    let point = clip
        .event_tracks
        .values()
        .flat_map(|track| track.events.iter())
        .find(|item| {
            string_id(&item.value.event)
                .is_some_and(|value| value.contains("b9e2bd85-40978ab9-277e1a94-a3ee4e3c"))
        })
        .expect("roll camera shake point event must exist");
    assert!((point.value.start_time - 0.1).abs() < 1e-6);
    assert_eq!(point.value.start_time, point.value.end_time);
}

#[test]
fn montage_state_is_mapped_into_its_runtime_sequence_graph_clip() {
    let clip = load(ROLL);
    let sampled = sample_tracks(clip.event_tracks.values(), 0.5);
    assert!(sampled.iter().any(|event| {
        string_id(&event.event).is_some_and(|value| value.contains("am_als_roll.am_als_roll"))
    }));
}

#[test]
fn sprint_tracks_keep_all_source_point_events() {
    let clip = load(SPRINT);
    let events = clip
        .event_tracks
        .values()
        .flat_map(|track| track.events.iter())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4);
    assert!(
        events
            .iter()
            .any(|item| (item.value.start_time - 0.066667).abs() < 1e-6)
    );
    assert!(
        events
            .iter()
            .any(|item| (item.value.start_time - 0.366667).abs() < 1e-6)
    );
}

#[test]
fn clip_node_emits_the_graph_clip_event_queue_at_the_current_clip_time() {
    let event_id = "als_notify:test:state".to_string();
    let track = EventTrack {
        name: "als_notify:test".to_string(),
        events: vec![TrackItem::new(TrackItemValue {
            event: AnimationEvent::StringId(event_id.clone()),
            start_time: 0.0,
            end_time: 1.5,
        })],
    };
    let expected = EventQueue::with_events([SampledEvent {
        event: AnimationEvent::StringId(event_id),
        weight: 1.0,
        percentage: 0.5,
        track: Some(track.name.clone()),
    }]);
    assert_clip_queue(track, TimeUpdate::Absolute(0.75), expected);
}

#[test]
fn clip_node_seeks_to_the_requested_event_percentage() {
    let event_id = "als_notify:test:seek".to_string();
    let track = EventTrack {
        name: "als_notify:seek".to_string(),
        events: vec![TrackItem::new(TrackItemValue {
            event: AnimationEvent::StringId(event_id.clone()),
            start_time: 0.0,
            end_time: 1.5,
        })],
    };
    let expected = EventQueue::with_events([SampledEvent {
        event: AnimationEvent::StringId(event_id.clone()),
        weight: 1.0,
        percentage: 0.5,
        track: Some(track.name.clone()),
    }]);
    assert_clip_queue(
        track,
        TimeUpdate::PercentOfEvent {
            percent: 0.5,
            event: AnimationEvent::StringId(event_id),
            track: "als_notify:seek".into(),
        },
        expected,
    );
}

#[test]
fn independent_clip_contexts_keep_event_queues_separate() {
    let first_id = "als_notify:test:first".to_string();
    let first_track = EventTrack {
        name: "als_notify:first".to_string(),
        events: vec![TrackItem::new(TrackItemValue {
            event: AnimationEvent::StringId(first_id.clone()),
            start_time: 0.0,
            end_time: 1.5,
        })],
    };
    let first_expected = EventQueue::with_events([SampledEvent {
        event: AnimationEvent::StringId(first_id),
        weight: 1.0,
        percentage: 0.5,
        track: Some(first_track.name.clone()),
    }]);
    assert_clip_queue(first_track, TimeUpdate::Absolute(0.75), first_expected);

    let second_id = "als_notify:test:second".to_string();
    let second_track = EventTrack {
        name: "als_notify:second".to_string(),
        events: vec![TrackItem::new(TrackItemValue {
            event: AnimationEvent::StringId(second_id.clone()),
            start_time: 0.0,
            end_time: 1.5,
        })],
    };
    let second_expected = EventQueue::with_events([SampledEvent {
        event: AnimationEvent::StringId(second_id),
        weight: 1.0,
        percentage: 0.5,
        track: Some(second_track.name.clone()),
    }]);
    assert_clip_queue(second_track, TimeUpdate::Absolute(0.75), second_expected);
}

#[test]
fn root_motion_clip_forwards_graph_clip_events() {
    let event_id = "als_notify:test:root_motion".to_string();
    let track = EventTrack {
        name: "als_notify:root_motion".to_string(),
        events: vec![TrackItem::new(TrackItemValue {
            event: AnimationEvent::StringId(event_id.clone()),
            start_time: 0.0,
            end_time: 1.5,
        })],
    };
    let handle = Handle::<GraphClip>::default();
    let expected = EventQueue::with_events([SampledEvent {
        event: AnimationEvent::StringId(event_id),
        weight: 1.0,
        percentage: 0.5,
        track: Some(track.name.clone()),
    }]);
    GraphTestHarness::new(ClipExtractor {
        node: AnimationNode::new(
            "root_motion_clip",
            RootMotionClipNode {
                clip: handle.clone(),
                root_motion_mode: RootMotionMode::Disabled,
                root_motion_bone: None,
            },
        ),
        handle,
        track,
    })
    .with_time_fwd(Some(TimeUpdate::Absolute(0.75)))
    .when_queried()
    .then_output_is("events", expected);
}

#[test]
fn every_compiled_event_track_deserializes_and_preserves_the_manifest_count() {
    let manifest_path = assets_root().join("animation_events/als_notify_manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(manifest_path).expect("notify manifest must be readable"),
    )
    .expect("notify manifest must be valid json");
    let mut graph_clips = BTreeSet::new();
    let mut expected_events = 0usize;
    for source in manifest["clips"]
        .as_array()
        .expect("clips must be an array")
    {
        let events = source["events"]
            .as_array()
            .expect("events must be an array");
        if events.is_empty() {
            continue;
        }
        expected_events += events.len();
        for segment in source["runtime_segments"]
            .as_array()
            .expect("runtime segments must be an array")
        {
            graph_clips.insert(
                segment["graph_clip"]
                    .as_str()
                    .expect("runtime graph clip must be a string")
                    .to_string(),
            );
        }
    }
    let compiled_events = graph_clips
        .iter()
        .map(|path| {
            load(path)
                .event_tracks
                .values()
                .map(|track| track.events.len())
                .sum::<usize>()
        })
        .sum::<usize>();
    assert_eq!(compiled_events, expected_events);
    assert!(!graph_clips.is_empty());
}
