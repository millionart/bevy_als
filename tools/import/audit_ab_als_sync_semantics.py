import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from audit_ab_als_animation_semantics import (
    TARGETS,
    asset_leaf,
    enum_value,
    export_class,
    node_tagged,
    numeric,
    outer_path,
)
from uasset_base import UAsset
from uasset_reader import PropReader


PLAYER_CLASSES = {
    "AnimGraphNode_SequencePlayer": "sequence_player",
    "AnimGraphNode_SequenceEvaluator": "sequence_evaluator",
    "AnimGraphNode_BlendSpacePlayer": "blendspace_player",
    "AnimGraphNode_BlendSpaceEvaluator": "blendspace_evaluator",
}


def marker_rows(value):
    if not isinstance(value, list):
        return []
    rows = []
    for marker in value:
        if not isinstance(marker, dict):
            continue
        name = str(marker.get("MarkerName", "None"))
        time = numeric(marker.get("Time"), 0.0)
        if name != "None":
            rows.append((name, float(time)))
    return rows


def animation_assets(source_root):
    result = {}
    for path in source_root.rglob("*.uasset"):
        asset = UAsset(str(path))
        reader = PropReader(asset)
        for index, export in enumerate(asset.exports):
            if export_class(asset, export.class_index) != "AnimSequence":
                continue
            properties, ok, _ = reader.read_export(index)
            if not ok:
                continue
            result[export.object_name] = {
                "length": float(numeric(properties.get("SequenceLength"), 0.0)),
                "markers": marker_rows(properties.get("AuthoredSyncMarkers")),
            }
    return result


def interpolation_value(value):
    if isinstance(value, dict):
        tagged = value.get("__tagged__", value)
        if isinstance(tagged, dict):
            return {
                "time": float(numeric(tagged.get("InterpolationTime"), 0.0)),
                "type": enum_value(tagged.get("InterpolationType", "None")),
            }
    return {"time": 0.0, "type": "None"}


def blendspace_assets(source_root, animations):
    result = {}
    for path in source_root.rglob("*.uasset"):
        asset = UAsset(str(path))
        reader = PropReader(asset)
        for index, export in enumerate(asset.exports):
            if export_class(asset, export.class_index) != "BlendSpace":
                continue
            properties, ok, _ = reader.read_export(index)
            if not ok:
                continue
            samples = []
            for sample in properties.get("SampleData", []):
                if not isinstance(sample, dict):
                    continue
                name = asset_leaf(sample.get("Animation"))
                if not name:
                    continue
                animation = animations.get(name, {})
                samples.append(
                    {
                        "asset": name,
                        "length": animation.get("length", 0.0),
                        "markers": animation.get("markers", []),
                    }
                )
            result[export.object_name] = {
                "length": float(numeric(properties.get("AnimLength"), 0.0)),
                "interpolation": interpolation_value(properties.get("InterpolationParam")),
                "samples": samples,
            }
    return result


def source_rows(source_root, animations, blendspaces):
    rows = []
    scanned = set()
    for contract, relative, destination in TARGETS:
        path = source_root / relative
        asset = UAsset(str(path))
        scanned.add(contract)
        reader = PropReader(asset)
        for index, export in enumerate(asset.exports):
            kind = PLAYER_CLASSES.get(export_class(asset, export.class_index))
            if kind is None:
                continue
            properties, ok, _ = reader.read_export(index)
            if not ok:
                continue
            node = node_tagged(properties)
            is_sequence = kind in {"sequence_player", "sequence_evaluator"}
            source_asset = asset_leaf(node.get("Sequence") if is_sequence else node.get("BlendSpace"))
            group = str(node.get("GroupName", "None"))
            role = enum_value(node.get("GroupRole", "Default"))
            method = enum_value(node.get("Method", "DoNotSync"))
            start = float(numeric(node.get("StartPosition"), 0.0))
            rate = float(numeric(node.get("PlayRate"), 1.0))
            loop_key = "bLoopAnimation" if kind == "sequence_player" else "bShouldLoop" if kind == "sequence_evaluator" else "bLoop"
            loop = bool(node.get(loop_key, True))
            reset = bool(node.get("bResetPlayTimeWhenBlendSpaceChanges", True))
            detail = animations.get(source_asset, {}) if is_sequence else blendspaces.get(source_asset, {})
            interpolation = detail.get("interpolation", {"time": 0.0, "type": "None"})
            samples = detail.get("samples", [])
            markers = detail.get("markers", []) if is_sequence else []
            needs_mapping = (
                group != "None"
                or role != "Default"
                or method != "DoNotSync"
                or abs(start) > 1e-6
                or abs(interpolation["time"]) > 1e-6
                or bool(markers)
                or any(sample["markers"] for sample in samples)
            )
            rows.append(
                {
                    "contract": contract,
                    "destination": destination,
                    "kind": kind,
                    "outer": outer_path(asset, export),
                    "asset": source_asset or "None",
                    "group": group,
                    "role": role,
                    "method": method,
                    "loop": loop,
                    "rate": rate,
                    "start": start,
                    "reset_on_asset_change": reset,
                    "asset_length": detail.get("length", 0.0),
                    "interpolation_time": interpolation["time"],
                    "interpolation_type": interpolation["type"],
                    "markers": markers,
                    "samples": samples,
                    "needs_mapping": needs_mapping,
                }
            )
    return rows, scanned


def destination_capabilities(bevy_root):
    text = "\n".join(path.read_text(encoding="utf-8") for path in bevy_root.rglob("*.ron"))
    return {
        "sync_group": "SyncGroup" in text,
        "marker": "Marker" in text,
        "normalized_phase": "NormalizedPhase" in text,
        "blendspace_filter": "BlendSpaceFilter" in text,
    }


def marker_text(markers):
    return ", ".join(f"{name}@{time:.6f}" for name, time in markers) or "None"


def sample_text(samples):
    return "; ".join(
        f"{sample['asset']}:{sample['length']:.6f}[{marker_text(sample['markers'])}]"
        for sample in samples
    ) or "None"


def reason(row):
    values = []
    if row["group"] != "None":
        values.append(f"group={row['group']}")
    if row["role"] != "Default":
        values.append(f"role={row['role']}")
    if row["method"] != "DoNotSync":
        values.append(f"method={row['method']}")
    if abs(row["start"]) > 1e-6:
        values.append(f"start={row['start']:.6f}")
    if abs(row["interpolation_time"]) > 1e-6:
        values.append(
            f"filter={row['interpolation_time']:.6f}/{row['interpolation_type']}"
        )
    if row["markers"] or any(sample["markers"] for sample in row["samples"]):
        values.append("markers")
    return ", ".join(values) or "none"


def markdown(rows, capabilities, scanned):
    mapped = capabilities["sync_group"] and capabilities["marker"] and capabilities["normalized_phase"]
    required = [row for row in rows if row["needs_mapping"]]
    unresolved = required if not mapped else []
    lines = [
        "# ALS AnimBP Sync and Phase Audit",
        "",
        "This report is generated from source AnimBlueprint uassets supplied on the command line and the checked-in AB_Als graph tree.",
        "",
        "## Verdict",
        "",
        f"- Source timing nodes: {len(rows)}",
        f"- Source nodes requiring timing mapping: {len(required)}",
        f"- Explicit unresolved mappings: {len(unresolved)}",
        f"- Destination SyncGroup capability: {capabilities['sync_group']}",
        f"- Destination marker capability: {capabilities['marker']}",
        f"- Destination normalized phase capability: {capabilities['normalized_phase']}",
        f"- Destination BlendSpace filter capability: {capabilities['blendspace_filter']}",
        "",
        "## Source Timing Inventory",
        "",
        "| Contract | Kind | Source graph | Asset | Group/role/method | Loop/rate/start/reset | Asset length | BlendSpace filter | Markers or samples | Required mapping | Status |",
        "| --- | --- | --- | --- | --- | --- | ---: | --- | --- | --- | --- |",
    ]
    for row in rows:
        sync = f"{row['group']} / {row['role']} / {row['method']}"
        playback = (
            f"{row['loop']} / {row['rate']:.6f} / {row['start']:.6f} / "
            f"{row['reset_on_asset_change']}"
        )
        filter_value = f"{row['interpolation_time']:.6f} / {row['interpolation_type']}"
        timing = marker_text(row["markers"]) if "sequence" in row["kind"] else sample_text(row["samples"])
        status = "UNRESOLVED" if row in unresolved else "Recorded"
        lines.append(
            f"| {row['contract']} | {row['kind']} | {row['outer']} | {row['asset']} | {sync} | {playback} | {row['asset_length']:.6f} | {filter_value} | {timing} | {reason(row)} | {status} |"
        )
    coverage = bool(rows) and scanned == {contract for contract, _, _ in TARGETS} and all(
        row["asset"] != "None"
        and row["kind"] in PLAYER_CLASSES.values()
        and isinstance(row["group"], str)
        and isinstance(row["role"], str)
        and isinstance(row["method"], str)
        for row in rows
    )
    lines.extend(
        [
            "",
            "## Result",
            "",
            "- Coverage: PASS" if coverage else "- Coverage: FAIL",
            f"- Parity: {'PASS' if not unresolved else 'PENDING'}",
            "",
        ]
    )
    return "\n".join(lines), coverage


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--bevy-root", type=Path, default=Path("assets/graphs/ab_als"))
    parser.add_argument("--report", type=Path, default=Path("docs/sync-01-source-audit.md"))
    args = parser.parse_args()
    animations = animation_assets(args.source_root)
    blendspaces = blendspace_assets(args.source_root, animations)
    rows, scanned = source_rows(args.source_root, animations, blendspaces)
    capabilities = destination_capabilities(args.bevy_root)
    report, coverage = markdown(rows, capabilities, scanned)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(report, encoding="utf-8", newline="\n")
    print(
        json.dumps(
            {
                "report": args.report.as_posix(),
                "source_timing_nodes": len(rows),
                "requires_mapping": sum(row["needs_mapping"] for row in rows),
                "coverage_pass": coverage,
                "parity_pass": capabilities["sync_group"]
                and capabilities["marker"]
                and capabilities["normalized_phase"],
            },
            indent=2,
        )
    )
    return 0 if coverage else 1


if __name__ == "__main__":
    raise SystemExit(main())
