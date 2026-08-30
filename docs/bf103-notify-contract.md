# BF103 Notify Asset Contract

## Source of Truth

`docs/bf103-notify-source-audit.json` is the checked-in, project-owned record of the local ALS source audit. It preserves source asset identity, event identity, class, parameters, timing, track, filters, trigger weight, montage segment mapping and authored sync markers. It contains no Unreal package payload.

The generator introduced in BF103.02 must reproduce all records from the audit before producing runtime assets. It must reject a missing source record, duplicate event identifier, invalid range, unresolved local notify class, invalid montage segment or a sequence reference that is not mapped to an exported GraphClip.

## Derived Event Manifest

BF103.02 will generate `assets/animation_events/als_notify_manifest.json` with this schema.

```json
{
  "version": 1,
  "clips": [
    {
      "source_asset": "/ALS/...",
      "source_kind": "AnimSequence",
      "graph_clip": "animations/.../A_Als_Example.anim.ron",
      "events": [
        {
          "id": "source-guid",
          "event_id": "als_notify:source-guid",
          "kind": "point",
          "name": "AlsCameraShake",
          "class": "AlsAnimNotify_CameraShake",
          "track": "Notify",
          "start_time": 0.1,
          "end_time": 0.1,
          "weight_threshold": 0.5,
          "filter": {
            "type": "ENotifyFilterType::NoFiltering",
            "lod": 0,
            "trigger_on_follower": false,
            "trigger_on_dedicated_server": false
          },
          "parameters": {}
        }
      ]
    }
  ]
}
```

`parameters` retains the source object fields as JSON values. The source `start_time` and `end_time` remain exact. A point event must not be widened or shifted to compensate for a frame rate.

## Runtime Compilation

Each derived interval record compiles to the corresponding GraphClip `event_tracks` entry and uses its stable `event_id` as `AnimationEvent::StringId`. The GraphClip data stays with the clip and is sampled by the actual `ClipNode` time path.

`EventTrack` samples active intervals. BF103.04 must therefore retain the exact point timestamps from the manifest and emit a point event only when the real clip time crosses that timestamp. It must not represent a point event as a synthetic interval. Notify State records retain their source interval and the lifecycle adapter derives Begin, Active and End from the sampled track state.

Montage records remain separate source clips until their slot segment maps to an actual exported sequence GraphClip. The mapping preserves slot, segment start, sequence start and end, play rate and loop count. No character-wide clock is permitted.

## Marker Separation

`AuthoredSyncMarkers` are synchronization-phase data, not automatically gameplay Notify events. Their source data remains canonical in `assets/sync/als_sync_metadata.ron` and the generic sync mechanism in `crates/als-ext`. BF103.04 may expose a crossed-marker event only when the source consumer requires it; it must preserve the marker-aware sync path rather than replace it with a global timer.

## Ownership

The manifest-to-EventTrack compiler, event queue bridge and lifecycle logic are reusable mechanisms in `crates/als-ext`. ALS class names, parameter mapping, montage selection and gameplay or camera consumers remain in the ALS project crates.
