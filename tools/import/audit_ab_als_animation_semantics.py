import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from uasset_base import ParseError, UAsset
from uasset_reader import PropReader


TARGETS = [
    ("ALI_Overlay", "Character/ALI_Overlay.uasset", "ali_overlay.animgraph.ron"),
    ("AB_Als", "Character/AB_Als.uasset", "ab_als.animgraph.ron"),
    ("AB_Als_Grounded", "Character/AnimationInstances/AB_Als_Grounded.uasset", "grounded.animgraph.ron"),
    ("AB_Als_Head", "Character/AnimationInstances/AB_Als_Head.uasset", "head.animgraph.ron"),
    ("AB_Als_Layering", "Character/AnimationInstances/AB_Als_Layering.uasset", "layering.animgraph.ron"),
    ("AB_Als_Locomotion", "Character/AnimationInstances/AB_Als_Locomotion.uasset", "locomotion.animgraph.ron"),
    ("AB_Als_Ragdolling", "Character/AnimationInstances/AB_Als_Ragdolling.uasset", "ragdolling.animgraph.ron"),
    ("AB_Als_View", "Character/AnimationInstances/AB_Als_View.uasset", "view.animgraph.ron"),
    ("AB_Als_Barrel", "Character/AnimationInstances/Overlays/AB_Als_Barrel.uasset", "overlays/barrel.animgraph.ron"),
    ("AB_Als_Binoculars", "Character/AnimationInstances/Overlays/AB_Als_Binoculars.uasset", "overlays/binoculars.animgraph.ron"),
    ("AB_Als_Bow", "Character/AnimationInstances/Overlays/AB_Als_Bow.uasset", "overlays/bow.animgraph.ron"),
    ("AB_Als_Box", "Character/AnimationInstances/Overlays/AB_Als_Box.uasset", "overlays/box.animgraph.ron"),
    ("AB_Als_Default", "Character/AnimationInstances/Overlays/AB_Als_Default.uasset", "overlays/default.animgraph.ron"),
    ("AB_Als_Feminine", "Character/AnimationInstances/Overlays/AB_Als_Feminine.uasset", "overlays/feminine.animgraph.ron"),
    ("AB_Als_HandsTied", "Character/AnimationInstances/Overlays/AB_Als_HandsTied.uasset", "overlays/hands_tied.animgraph.ron"),
    ("AB_Als_Injured", "Character/AnimationInstances/Overlays/AB_Als_Injured.uasset", "overlays/injured.animgraph.ron"),
    ("AB_Als_Masculine", "Character/AnimationInstances/Overlays/AB_Als_Masculine.uasset", "overlays/masculine.animgraph.ron"),
    ("AB_Als_PistolOneHanded", "Character/AnimationInstances/Overlays/AB_Als_PistolOneHanded.uasset", "overlays/pistol_one_handed.animgraph.ron"),
    ("AB_Als_PistolTwoHanded", "Character/AnimationInstances/Overlays/AB_Als_PistolTwoHanded.uasset", "overlays/pistol_two_handed.animgraph.ron"),
    ("AB_Als_Rifle", "Character/AnimationInstances/Overlays/AB_Als_Rifle.uasset", "overlays/rifle.animgraph.ron"),
    ("AB_Als_Torch", "Character/AnimationInstances/Overlays/AB_Als_Torch.uasset", "overlays/torch.animgraph.ron"),
    ("AB_Als_Crouching", "Character/AnimationInstances/Stances/AB_Als_Crouching.uasset", "stances/crouching.animgraph.ron"),
    ("AB_Als_Standing", "Character/AnimationInstances/Stances/AB_Als_Standing.uasset", "stances/standing.animgraph.ron"),
]

SEQUENCE_PLAYER_CLASSES = {
    "AnimGraphNode_SequencePlayer",
    "AnimGraphNode_SequencePlayerBase",
}
SEQUENCE_EVALUATOR_CLASSES = {
    "AnimGraphNode_SequenceEvaluator",
    "AnimGraphNode_SequenceEvaluatorBase",
}
BLENDSPACE_CLASSES = {
    "AnimGraphNode_BlendSpacePlayer",
    "AnimGraphNode_BlendSpaceEvaluator",
}


def outer_path(asset, export):
    parts = []
    current = export.outer_index
    while current > 0:
        outer = asset.exports[current - 1]
        parts.append(outer.object_name)
        current = outer.outer_index
    return "/".join(reversed(parts))


def export_class(asset, class_index):
    if class_index < 0:
        imported = asset.imports[-class_index - 1]
        return imported.object_name if imported.class_name == "Class" else imported.class_name
    return None


def node_tagged(properties):
    node = properties.get("Node")
    if not isinstance(node, dict):
        return {}
    tagged = node.get("__tagged__")
    return tagged if isinstance(tagged, dict) else {}


def asset_leaf(value):
    if not isinstance(value, str):
        return None
    leaf = value.rsplit("/", 1)[-1]
    return leaf if leaf and leaf != "None" else None


def numeric(value, fallback):
    return value if isinstance(value, (int, float)) else fallback


def enum_value(value):
    if isinstance(value, dict):
        return str(value.get("value", value.get("enum_name", "None")))
    return str(value) if value is not None else "None"


def source_records(source_root):
    result = {}
    for contract, relative, destination in TARGETS:
        source_path = source_root / relative
        if not source_path.is_file():
            raise FileNotFoundError(f"missing source asset: {relative}")
        asset = UAsset(str(source_path))
        reader = PropReader(asset)
        players = []
        transitions = []
        for index, export in enumerate(asset.exports):
            cls = export_class(asset, export.class_index)
            if cls not in SEQUENCE_PLAYER_CLASSES | SEQUENCE_EVALUATOR_CLASSES | BLENDSPACE_CLASSES | {"AnimStateTransitionNode"}:
                continue
            properties, ok, _ = reader.read_export(index)
            if not ok:
                continue
            node = node_tagged(properties)
            if cls in SEQUENCE_PLAYER_CLASSES:
                players.append(
                    {
                        "kind": "sequence_player",
                        "outer": outer_path(asset, export),
                        "asset": asset_leaf(node.get("Sequence")),
                        "loop": bool(node.get("bLoopAnimation", True)),
                        "play_rate": numeric(node.get("PlayRate"), 1.0),
                        "start_position": numeric(node.get("StartPosition"), 0.0),
                        "sync_group": str(node.get("GroupName", "None")),
                        "sync_method": enum_value(node.get("Method", "DoNotSync")),
                    }
                )
            elif cls in SEQUENCE_EVALUATOR_CLASSES:
                players.append(
                    {
                        "kind": "sequence_evaluator",
                        "outer": outer_path(asset, export),
                        "asset": asset_leaf(node.get("Sequence")),
                        "loop": bool(node.get("bShouldLoop", True)),
                        "play_rate": 1.0,
                        "start_position": numeric(node.get("StartPosition"), 0.0),
                        "sync_group": str(node.get("GroupName", "None")),
                        "sync_method": enum_value(node.get("Method", "DoNotSync")),
                        "explicit_frame": numeric(node.get("ExplicitFrame"), 0),
                        "explicit_time": numeric(node.get("ExplicitTime"), 0.0),
                        "teleport": bool(node.get("bTeleportToExplicitTime", True)),
                    }
                )
            elif cls in BLENDSPACE_CLASSES:
                players.append(
                    {
                        "kind": "blendspace_evaluator" if cls.endswith("Evaluator") else "blendspace_player",
                        "outer": outer_path(asset, export),
                        "asset": asset_leaf(node.get("BlendSpace")),
                        "loop": bool(node.get("bLoop", True)),
                        "play_rate": numeric(node.get("PlayRate"), 1.0),
                        "start_position": numeric(node.get("StartPosition"), 0.0),
                        "sync_group": str(node.get("GroupName", "None")),
                        "sync_method": enum_value(node.get("Method", "DoNotSync")),
                        "asset_ref": node.get("BlendSpace"),
                    }
                )
            else:
                transitions.append(
                    {
                        "outer": outer_path(asset, export),
                        "duration": numeric(node.get("CrossfadeDuration"), 0.0),
                        "blend_mode": enum_value(node.get("BlendMode", "Linear")),
                        "automatic_time": numeric(node.get("AutomaticRuleTriggerTime"), -1.0),
                    }
                )
        result[contract] = {
            "relative_source": relative,
            "destination": destination,
            "players": players,
            "transitions": transitions,
        }
    return result


def blendspace_properties(source_root, source):
    index = {path.stem: path for path in source_root.rglob("*.uasset")}
    cache = {}
    for data in source.values():
        for record in data["players"]:
            if not record["kind"].startswith("blendspace"):
                continue
            asset_name = record["asset"]
            if not asset_name:
                continue
            if asset_name not in cache:
                path = index.get(asset_name)
                if path is None:
                    cache[asset_name] = {"axis": "Missing", "samples": 0, "asset_loop": "Missing"}
                else:
                    asset = UAsset(str(path))
                    reader = PropReader(asset)
                    properties, ok, _ = reader.read_export(0)
                    if not ok:
                        cache[asset_name] = {"axis": "Unreadable", "samples": 0, "asset_loop": "Unreadable"}
                    else:
                        parameters = properties.get("BlendParameters", {})
                        tagged = parameters.get("__tagged__", {}) if isinstance(parameters, dict) else {}
                        axis = "; ".join(
                            f"{entry.get('DisplayName', 'Axis')}[{entry.get('Min', 0)},{entry.get('Max', 0)}]"
                            for entry in (tagged if isinstance(tagged, list) else [tagged])
                            if isinstance(entry, dict)
                        )
                        sample_data = properties.get("SampleData", [])
                        coordinates = [
                            item.get("SampleValue", {})
                            for item in sample_data
                            if isinstance(item, dict)
                        ] if isinstance(sample_data, list) else []
                        ranges = []
                        for key in ("x", "y"):
                            values = [value[key] for value in coordinates if isinstance(value, dict) and isinstance(value.get(key), (int, float))]
                            if values:
                                ranges.append(f"{key.upper()}[{min(values)},{max(values)}]")
                        cache[asset_name] = {
                            "axis": f"{axis or 'None'}; {' '.join(ranges)}",
                            "samples": len(sample_data) if isinstance(sample_data, list) else 0,
                            "asset_loop": bool(properties.get("bLoop", False)),
                        }
            record["blendspace"] = cache[asset_name]


def destination_records(bevy_root):
    clips = defaultdict(list)
    time_positions = []
    blendspaces = []
    fsms = []
    loop_nodes = []
    blend_nodes = []
    time_pattern = re.compile(
        r'name:\s*"([^"]+)"\s*,\s*inner:\s*\{\s*"als_ext::TimePositionNode"\s*:\s*\((.*?)\)\s*\}',
        re.DOTALL,
    )
    clip_pattern = re.compile(r'ClipNode"\s*:\s*\(\s*clip:\s*"([^"]+)"', re.DOTALL)
    for path in sorted(bevy_root.rglob("*.ron")):
        relative = path.relative_to(bevy_root).as_posix()
        text = path.read_text(encoding="utf-8")
        for clip in clip_pattern.findall(text):
            leaf = Path(clip).name.removesuffix(".anim.ron")
            clips[leaf].append(relative)
        for name, options in time_pattern.findall(text):
            if "loop_position: true" in options:
                loop = True
            elif "loop_position: false" in options:
                loop = False
            else:
                loop = None
            time_positions.append({"graph": relative, "name": name, "loop": loop})
        if "BlendSpaceNode" in text or "BlendSpace1DNode" in text:
            blendspaces.append(relative)
            for line in text.splitlines():
                if "BlendSpaceNode" in line or "BlendSpace1DNode" in line:
                    blend_nodes.append({"graph": relative, "node": line.strip()})
        if "InterruptibleFsmNode" in text:
            fsms.append(relative)
        if "LoopNode" in text:
            loop_nodes.append(relative)
    return {
        "clips": clips,
        "time_positions": time_positions,
        "blendspaces": blendspaces,
        "fsms": fsms,
        "loop_nodes": loop_nodes,
        "blend_nodes": blend_nodes,
    }


def summary(contract, source, destination):
    players = source["players"]
    seq = [row for row in players if row["kind"].startswith("sequence")]
    blend = [row for row in players if row["kind"].startswith("blendspace")]
    sequence_assets = sorted({row["asset"] for row in seq if row["asset"]})
    missing = [asset for asset in sequence_assets if asset not in destination["clips"]]
    loop_true = sum(1 for row in players if row["loop"])
    loop_false = len(players) - loop_true
    rates = sum(1 for row in players if abs(float(row["play_rate"]) - 1.0) > 1e-6)
    starts = sum(1 for row in players if abs(float(row["start_position"])) > 1e-6)
    groups = sorted({row["sync_group"] for row in players if row["sync_group"] != "None"})
    return {
        "contract": contract,
        "source_players": len(players),
        "source_sequences": len(seq),
        "source_blendspaces": len(blend),
        "source_transitions": len(source["transitions"]),
        "loop_true": loop_true,
        "loop_false": loop_false,
        "non_default_rates": rates,
        "non_zero_starts": starts,
        "sync_groups": groups,
        "source_assets": sequence_assets,
        "missing_assets": missing,
        "destination": source["destination"],
    }


def markdown(source, destination):
    rows = [summary(contract, data, destination) for contract, data in source.items()]
    missing = sorted({asset for row in rows for asset in row["missing_assets"]})
    time_counts = Counter(
        "true" if row["loop"] is True else "false" if row["loop"] is False else "unspecified"
        for row in destination["time_positions"]
    )
    lines = [
        "# BF100-24 Animation Resource, Loop, and Blend Audit",
        "",
        "This report is generated by `tools/import/audit_ab_als_animation_semantics.py` from the source uassets supplied on the command line and the checked-in AB_Als graph tree.",
        "",
        "## Verdict",
        "",
        f"- Source sequence assets without a Bevy ClipNode reference: {len(missing)}",
        f"- Destination TimePositionNode loop=true: {time_counts['true']}",
        f"- Destination TimePositionNode loop=false: {time_counts['false']}",
        f"- Destination TimePositionNode loop unspecified: {time_counts['unspecified']}",
        f"- Destination LoopNode graphs: {len(destination['loop_nodes'])}",
        f"- Destination BlendSpace graphs: {len(destination['blendspaces'])}",
        f"- Destination InterruptibleFsm graphs: {len(destination['fsms'])}",
        "",
        "## Contract Summary",
        "",
        "| Contract | Players | Sequence Evaluators/Players | BlendSpaces | Transitions | Loop T/F | Non-default rate/start | Sync groups | Destination graph | Missing assets |",
        "| --- | ---: | ---: | ---: | ---: | --- | --- | --- | --- | --- |",
    ]
    for row in rows:
        groups = ", ".join(row["sync_groups"]) or "None"
        missing_asset_text = ", ".join(row["missing_assets"]) or "None"
        lines.append(
            f"| {row['contract']} | {row['source_players']} | {row['source_sequences']} | {row['source_blendspaces']} | {row['source_transitions']} | {row['loop_true']}/{row['loop_false']} | {row['non_default_rates']}/{row['non_zero_starts']} | {groups} | `{row['destination']}` | {missing_asset_text} |"
        )
    lines.extend(
        [
            "",
            "## Source Player and Evaluator Inventory",
            "",
            "| Contract | Kind | Source graph | Asset | Loop | Rate | Start | Sync group | Sync method | Explicit frame/time |",
            "| --- | --- | --- | --- | --- | ---: | ---: | --- | --- | --- |",
        ]
    )
    lines.extend(
        [
            "",
            "## Source BlendSpace Axis Inventory",
            "",
            "| Contract | Kind | Source BlendSpace | Axis range | Samples | Asset loop | Player loop | Rate | Destination graph scope |",
            "| --- | --- | --- | --- | ---: | --- | --- | ---: | --- |",
        ]
    )
    for contract, data in source.items():
        for row in data["players"]:
            if not row["kind"].startswith("blendspace"):
                continue
            spec = row.get("blendspace", {})
            lines.append(
                f"| {contract} | {row['kind']} | {row['asset']} | {spec.get('axis', 'None')} | {spec.get('samples', 0)} | {spec.get('asset_loop', 'None')} | {row['loop']} | {row['play_rate']} | `{data['destination']}` |"
            )
    lines.extend(
        [
            "",
            "## Destination BlendSpace Node Inventory",
            "",
            "| Graph | Node configuration |",
            "| --- | --- |",
        ]
    )
    for row in destination["blend_nodes"]:
        lines.append(f"| `{row['graph']}` | `{row['node']}` |")
    for contract, data in source.items():
        for row in data["players"]:
            explicit = ""
            if row["kind"] == "sequence_evaluator":
                explicit = f"{row.get('explicit_frame', 0)}/{row.get('explicit_time', 0.0)}"
            lines.append(
                "| {contract} | {kind} | {outer} | {asset} | {loop} | {play_rate} | {start_position} | {sync_group} | {sync_method} | {explicit} |".format(
                    contract=contract,
                    explicit=explicit,
                    **row,
                )
            )
    lines.extend(
        [
            "",
            "## Source State Transition Inventory",
            "",
            "| Contract | Source graph | Crossfade duration | Blend mode | Automatic trigger time |",
            "| --- | --- | ---: | --- | ---: |",
        ]
    )
    for contract, data in source.items():
        for row in data["transitions"]:
            lines.append(
                "| {contract} | {outer} | {duration} | {blend_mode} | {automatic_time} |".format(
                    contract=contract,
                    **row,
                )
            )
    lines.extend(
        [
            "",
            "## Destination Time Position Inventory",
            "",
            "| Graph | Node | Effective loop setting |",
            "| --- | --- | --- |",
        ]
    )
    for row in destination["time_positions"]:
        value = "true" if row["loop"] is True else "false" if row["loop"] is False else "unspecified"
        lines.append(f"| `{row['graph']}` | {row['name']} | {value} |")
    lines.extend(
        [
            "",
            "## Equivalence Notes",
            "",
            "- UE SequencePlayer and SequenceEvaluator defaults are loop=true. SequencePlayer looping is represented by Clip plus LoopNode. SequenceEvaluator explicit time is clamped by the UE runtime before its loop flag affects tick records, so its Bevy TimePositionNode always uses loop_position: false.",
            "- UE SyncGroup records are represented by shared clock edges and Absolute BlendSpace synchronization in the linked graph tree. The inventory retains every source GroupName and Method for review.",
            "- UE state transitions are represented by the source-indexed InterruptibleFsm and tag-blend contracts. The transition inventory retains every source CrossfadeDuration and BlendMode even where multiple source edges share one ECS/FSM boundary.",
            "- A PASS requires no missing source sequence asset, no destination TimePositionNode wrap setting, and no unspecified destination TimePositionNode setting.",
        ]
    )
    verdict = not missing and time_counts["true"] == 0 and time_counts["unspecified"] == 0
    lines.extend(["", f"## Result: {'PASS' if verdict else 'FAIL'}", ""])
    return "\n".join(lines), verdict


def clamp_evaluator_positions(bevy_root):
    pattern = re.compile(r'("als_ext::TimePositionNode"\s*:\s*\(([^)]*)\))', re.DOTALL)
    changed = []
    for path in sorted(bevy_root.rglob("*.ron")):
        text = path.read_text(encoding="utf-8")

        def replace(match):
            whole = match.group(1)
            options = re.sub(r",\s*,\s*loop_position:", ", loop_position:", match.group(2))
            if "loop_position: true" in options:
                options = options.replace("loop_position: true", "loop_position: false")
                return whole.replace(match.group(2), options)
            if "loop_position: false" in options:
                return whole.replace(match.group(2), options)
            trimmed = options.rstrip()
            separator = " " if trimmed.endswith(",") else ", "
            return whole.replace(match.group(2), f"{trimmed}{separator}loop_position: false")

        updated = pattern.sub(replace, text)
        if updated != text:
            path.write_text(updated, encoding="utf-8", newline="\n")
            changed.append(path.relative_to(bevy_root).as_posix())
    return changed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--bevy-root", type=Path, default=Path("assets/graphs/ab_als"))
    parser.add_argument("--report", type=Path, default=Path("docs/bf100-24-animation-audit.md"))
    parser.add_argument("--clamp-evaluator-positions", action="store_true")
    args = parser.parse_args()
    if args.clamp_evaluator_positions:
        changed = clamp_evaluator_positions(args.bevy_root)
        print(json.dumps({"clamped_time_position_nodes": changed}, indent=2))
    try:
        source = source_records(args.source_root)
    except ParseError as error:
        print(f"source parse failed: {error}", file=sys.stderr)
        return 2
    blendspace_properties(args.source_root, source)
    destination = destination_records(args.bevy_root)
    report, verdict = markdown(source, destination)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(report, encoding="utf-8", newline="\n")
    print(json.dumps({"report": args.report.as_posix(), "pass": verdict}, indent=2))
    return 0 if verdict else 1


if __name__ == "__main__":
    raise SystemExit(main())
