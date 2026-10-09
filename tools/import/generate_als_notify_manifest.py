import argparse
import json
from pathlib import Path


class ValidationError(Exception):
    pass


def graph_clip_path(target_gltf):
    prefix = "assets/"
    if not isinstance(target_gltf, str) or not target_gltf.startswith(prefix) or not target_gltf.endswith(".gltf"):
        raise ValidationError("invalid_target_gltf")
    return target_gltf[len(prefix):-len(".gltf")] + ".anim.ron"


def source_object_from_reference(reference):
    if not isinstance(reference, str) or not reference:
        return None
    source = reference.split("//", 1)[-1]
    if not source.startswith("/"):
        source = "/" + source
    if not source.startswith("/ALS/"):
        return None
    parts = [part for part in source.split("/") if part]
    if len(parts) < 2:
        return None
    package = "/".join(["", *parts[:-1]])
    return package + "." + parts[-1]


def event_class(event):
    detail = event.get("notify_state") if event.get("kind") == "state" else event.get("notify")
    if isinstance(detail, dict) and isinstance(detail.get("class"), str):
        return detail["class"]
    return event.get("name", "None")


def event_parameters(event):
    detail = event.get("notify_state") if event.get("kind") == "state" else event.get("notify")
    if not isinstance(detail, dict):
        return {}
    properties = detail.get("properties")
    return properties if isinstance(properties, dict) else {}


def runtime_segments(row, targets, assets_root):
    if row["asset_class"] == "AnimSequence":
        target = row.get("target_gltf")
        clip = graph_clip_path(target)
        if not (assets_root / clip).is_file():
            raise ValidationError("missing_graph_clip:" + row["source_object"])
        return [{"graph_clip": clip, "slot": None, "start_position": 0.0, "animation_start_time": 0.0, "animation_end_time": row["sequence_length"], "play_rate": 1.0, "loop_count": 1}]
    if row["asset_class"] != "AnimMontage":
        raise ValidationError("unsupported_animation_class:" + row["asset_class"])
    segments = []
    for segment in row.get("montage_segments", []):
        source_object = source_object_from_reference(segment.get("animation"))
        target = targets.get(source_object)
        if target is None:
            raise ValidationError("unmapped_montage_sequence:" + row["source_object"])
        clip = graph_clip_path(target)
        if not (assets_root / clip).is_file():
            raise ValidationError("missing_montage_graph_clip:" + row["source_object"])
        segments.append(
            {
                "graph_clip": clip,
                "slot": segment.get("slot"),
                "start_position": segment.get("start_position"),
                "animation_start_time": segment.get("animation_start_time"),
                "animation_end_time": segment.get("animation_end_time"),
                "play_rate": segment.get("play_rate"),
                "loop_count": segment.get("loop_count"),
            }
        )
    if not segments:
        raise ValidationError("missing_montage_segment:" + row["source_object"])
    return segments


def manifest(audit, assets_root):
    rows = audit.get("animations")
    if not isinstance(rows, list):
        raise ValidationError("missing_animations")
    targets = {
        row["source_object"]: row["target_gltf"]
        for row in rows
        if row.get("target_gltf")
    }
    clips = []
    event_ids = set()
    for row in rows:
        segments = runtime_segments(row, targets, assets_root)
        events = []
        for event in row.get("notifies", []):
            source_id = event.get("id")
            if not isinstance(source_id, str) or source_id == "None":
                raise ValidationError("missing_event_id:" + row["source_object"])
            event_id = "als_notify:" + row["source_object"].lower() + ":" + source_id.lower()
            if event_id in event_ids:
                raise ValidationError("duplicate_event_id:" + event_id)
            event_ids.add(event_id)
            start = event.get("start_time")
            end = event.get("end_time")
            if not isinstance(start, (int, float)) or not isinstance(end, (int, float)):
                raise ValidationError("invalid_event_time:" + event_id)
            if start < 0.0 or end < start or end > row["sequence_length"] + 0.0001:
                raise ValidationError("event_time_out_of_range:" + event_id)
            if str(event.get("filter_type")).removeprefix("ENotifyFilterType::") not in ("NoFiltering", "LOD"):
                raise ValidationError("unsupported_notify_filter:" + event_id)
            if str(event.get("montage_tick_type")).removeprefix("EMontageNotifyTickType::") not in ("Queued", "BranchingPoint"):
                raise ValidationError("unsupported_notify_tick_type:" + event_id)
            if type(event.get("filter_lod")) is not int or not -(2**31) <= event["filter_lod"] < 2**31:
                raise ValidationError("invalid_notify_filter_lod:" + event_id)
            events.append(
                {
                    "id": source_id,
                    "event_id": event_id,
                    "kind": event.get("kind"),
                    "name": event.get("name"),
                    "class": event_class(event),
                    "track": event.get("track_name"),
                    "track_index": event.get("track_index"),
                    "start_time": start + event.get("trigger_time_offset", 0.0),
                    "end_time": end + event.get("trigger_time_offset", 0.0) + event.get("end_trigger_time_offset", 0.0),
                    "trigger_time_offset": event.get("trigger_time_offset", 0.0),
                    "weight_threshold": event.get("weight_threshold"),
                    "filter": {
                        "type": event.get("filter_type"),
                        "lod": event.get("filter_lod"),
                        "trigger_on_follower": event.get("trigger_on_follower"),
                        "trigger_on_dedicated_server": event.get("trigger_on_dedicated_server"),
                        "trigger_chance": event.get("trigger_chance"),
                        "montage_tick_type": event.get("montage_tick_type"),
                    },
                    "parameters": event_parameters(event),
                    "linked_sequence": event.get("linked_sequence"),
                    "linked_montage": event.get("linked_montage"),
                }
            )
        markers = []
        for marker in row.get("markers", []):
            time = marker.get("time")
            if not isinstance(time, (int, float)) or time < 0.0 or time > row["sequence_length"] + 0.0001:
                raise ValidationError("marker_time_out_of_range:" + row["source_object"])
            markers.append(
                {
                    "id": marker.get("id"),
                    "name": marker.get("name"),
                    "time": time,
                    "track_index": marker.get("track_index"),
                }
            )
        clips.append(
            {
                "source_asset": row["source_object"],
                "source_kind": row["asset_class"],
                "sequence_length": row["sequence_length"],
                "runtime_segments": segments,
                "events": events,
                "sync_markers": markers,
            }
        )
    return {"version": 1, "clips": clips}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--audit", default=Path("docs/bf103-notify-source-audit.json"), type=Path)
    parser.add_argument("--assets-root", default=Path("assets"), type=Path)
    parser.add_argument("--output", default=Path("assets/ALS/Animations/als_notify_manifest.json"), type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    audit = json.loads(args.audit.read_text(encoding="utf-8"))
    result = manifest(audit, args.assets_root)
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != encoded:
            raise SystemExit("manifest_mismatch")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8", newline="\n")
    event_count = sum(len(clip["events"]) for clip in result["clips"])
    marker_count = sum(len(clip["sync_markers"]) for clip in result["clips"])
    print(json.dumps({"clips": len(result["clips"]), "events": event_count, "markers": marker_count}, sort_keys=True))


if __name__ == "__main__":
    main()
