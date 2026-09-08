import argparse
import json
import re
import struct
import sys
import tempfile
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT.parent / "animgraph_authoring"))

from uasset_base import UAsset
from uasset_reader import PropReader
import ue_animation_asset_resolver as source_resolver
from ue_animation_asset_resolver import (
    native_source_asset,
    source_asset_name,
    source_object_path,
)
from ue_exported_text import ExportedTextError, field as struct_field, parse_number, parse_struct


CLIP_NODE = "bevy_animation_graph::builtin_nodes::ClipNode"
LOCAL_ADDITIVE = "AAT_LocalSpaceBase"
MESH_ADDITIVE = "AAT_RotationOffsetMeshSpace"
NODE_PATTERN = re.compile(
    r'\(\s*id:\s*"(?P<id>[^"]+)",\s*name:\s*"(?P<node>[^"]+)",\s*inner:\s*\{\s*"'
    + re.escape(CLIP_NODE)
    + r'"\s*:\s*\(\s*clip:\s*"(?P<clip>[^"]+)"',
    re.DOTALL,
)
SEQUENCE_PLAYER_PATTERN = re.compile(
    r'(?P<prefix>"als_ext::SourceSequencePlayerNode"\s*:\s*\(\s*clip:\s*"(?P<clip>[^"]+)"\s*,\s*additive_reference:\s*)(?P<reference>None|Some\("[^"]+"\))(?P<middle>\s*,\s*additive_reference_position:\s*)(?P<position>-?[0-9.]+)(?:\s*,\s*additive_mesh_space:\s*(?P<mesh>true|false))?'
)
SEQUENCE_EVALUATOR_PATTERN = re.compile(
    r'"als_ext::SourceSequenceEvaluatorNode"\s*:\s*\(\s*clip:\s*"(?P<clip>[^"]+)"'
)


def export_class(asset, class_index):
    if class_index < 0:
        imported = asset.imports[-class_index - 1]
        return imported.object_name if imported.class_name == "Class" else imported.class_name
    return None


def tagged_enum(value):
    if isinstance(value, dict):
        return str(value.get("value", value.get("enum_name", "AAT_None")))
    return str(value)


def leaf(value):
    return str(value).rsplit("/", 1)[-1] if value else "None"


def numerator(value):
    if isinstance(value, dict):
        if "Numerator" in value:
            return float(value["Numerator"])
        if "__tagged__" in value:
            return numerator(value["__tagged__"])
    return 0.0


def native_property(source, name):
    values = [
        value.get("exported_text")
        for value in source.get("asset_properties", [])
        if value.get("name") == name and isinstance(value.get("exported_text"), str)
    ]
    if len(values) != 1:
        raise ValueError(f"source native property is missing or ambiguous: {name}")
    return values[0]


def native_animation_metadata(source, source_path):
    if (
        source.get("format") != "ue_asset_source_v1"
        or source.get("asset_path") != source_path
        or source.get("asset_class") != "/Script/Engine.AnimSequence"
    ):
        raise ValueError("source native animation asset is invalid")
    try:
        frame_rate = parse_struct(native_property(source, "SamplingFrameRate"))
        numerator_value = parse_number(struct_field(frame_rate, "Numerator"))
        denominator_value = parse_number(struct_field(frame_rate, "Denominator"))
        if denominator_value <= 0.0:
            raise ValueError("source native sample rate denominator is invalid")
        reference = native_property(source, "RefPoseSeq")
        reference_asset = (
            source_asset_name(source_object_path(reference))
            if reference not in {"", "None"}
            else "None"
        )
        return {
            "additive_type": native_property(source, "AdditiveAnimType"),
            "reference_type": native_property(source, "RefPoseType"),
            "reference_asset": reference_asset,
            "reference_frame": int(parse_number(native_property(source, "RefFrameIndex"))),
            "sequence_length": parse_number(native_property(source, "SequenceLength")),
            "sample_rate": numerator_value / denominator_value,
            "frame_count": int(parse_number(native_property(source, "NumFrames"))),
            "source": source_path,
        }
    except ExportedTextError as error:
        raise ValueError("source native animation metadata is invalid") from error


def raw_animation_metadata(path):
    asset = UAsset(str(path))
    reader = PropReader(asset)
    matches = [
        index
        for index, export in enumerate(asset.exports)
        if export_class(asset, export.class_index) == "AnimSequence"
        and export.object_name == path.stem
    ]
    if len(matches) != 1:
        raise ValueError(f"{path}: source animation sequence export is missing or ambiguous")
    props, ok, _ = reader.read_export(matches[0])
    if not ok:
        raise ValueError(f"{path}: source animation sequence properties are unreadable")
    return {
        "additive_type": tagged_enum(props.get("AdditiveAnimType", "AAT_None")),
        "reference_type": tagged_enum(props.get("RefPoseType", "None")),
        "reference_asset": leaf(props.get("RefPoseSeq")),
        "reference_frame": int(props.get("RefFrameIndex", 0)),
        "sequence_length": float(props.get("SequenceLength", 0.0)),
        "sample_rate": numerator(props.get("SamplingFrameRate", 0.0)),
        "frame_count": int(props.get("NumFrames", 0)),
        "source": path.as_posix(),
    }


def native_source_path(source_root, source_prefix, path):
    relative = path.relative_to(source_root).with_suffix("").as_posix()
    return f"{source_prefix.rstrip('/')}/{relative}.{path.stem}"


def source_animation_assets(source_root, names=None, source_prefix=None, source_reader=None):
    if names is not None:
        if not source_prefix:
            raise ValueError("source native prefix is unavailable")
        available = {}
        for path in source_root.rglob("*.uasset"):
            available.setdefault(path.stem, []).append(path)
        result = {}
        pending = set(names)
        while pending:
            name = pending.pop()
            if name in result:
                continue
            matches = available.get(name, [])
            if len(matches) != 1:
                raise ValueError(f"source animation asset {name} resolves to {len(matches)} files")
            path = matches[0]
            try:
                metadata = raw_animation_metadata(path)
            except Exception:
                source_path = native_source_path(source_root, source_prefix, path)
                metadata = native_animation_metadata(
                    native_source_asset(source_path, source_reader), source_path
                )
            result[name] = metadata
            if (
                metadata["additive_type"] != "AAT_None"
                and metadata["reference_asset"] != "None"
            ):
                pending.add(metadata["reference_asset"])
        return result
    result = {}
    for path in source_root.rglob("*.uasset"):
        asset = UAsset(str(path))
        reader = PropReader(asset)
        for index, export in enumerate(asset.exports):
            if export_class(asset, export.class_index) != "AnimSequence":
                continue
            props, ok, _ = reader.read_export(index)
            if not ok:
                continue
            result[export.object_name] = {
                "additive_type": tagged_enum(props.get("AdditiveAnimType", "AAT_None")),
                "reference_type": tagged_enum(props.get("RefPoseType", "None")),
                "reference_asset": leaf(props.get("RefPoseSeq")),
                "reference_frame": int(props.get("RefFrameIndex", 0)),
                "sequence_length": float(props.get("SequenceLength", 0.0)),
                "sample_rate": numerator(props.get("SamplingFrameRate", 0.0)),
                "frame_count": int(props.get("NumFrames", 0)),
                "source": path.relative_to(source_root).as_posix(),
            }
    return result


def graph_clip_uses(graph_root):
    rows = []
    for path in sorted(graph_root.rglob("*.ron")):
        text = path.read_text(encoding="utf-8")
        for match in NODE_PATTERN.finditer(text):
            rows.append(
                {
                    "path": path,
                    "graph": path.relative_to(graph_root).as_posix(),
                    "node_id": match.group("id"),
                    "node": match.group("node"),
                    "clip_path": match.group("clip"),
                    "clip": Path(match.group("clip")).name.removesuffix(".anim.ron"),
                }
            )
    return rows


def read_graph(path):
    raw = path.read_bytes()
    newline = "\r\n" if b"\r\n" in raw else "\n"
    return raw.decode("utf-8"), newline


def raw_scale_summary(path):
    document = json.loads(path.read_text(encoding="utf-8"))
    buffers = []
    for buffer in document.get("buffers", []):
        uri = buffer.get("uri")
        buffers.append((path.parent / uri).read_bytes() if uri else b"")
    values = []
    for animation in document.get("animations", []):
        for channel in animation.get("channels", []):
            if channel.get("target", {}).get("path") != "scale":
                continue
            accessor = document["accessors"][animation["samplers"][channel["sampler"]]["output"]]
            if accessor.get("componentType") != 5126 or accessor.get("type") != "VEC3":
                continue
            view = document["bufferViews"][accessor["bufferView"]]
            data = buffers[view.get("buffer", 0)]
            offset = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
            stride = view.get("byteStride", 12)
            for index in range(accessor.get("count", 0)):
                values.append(struct.unpack_from("<3f", data, offset + index * stride))
    if not values:
        return {"scale_keys": 0, "scale_min": "None", "scale_max": "None", "unit": True}
    return {
        "scale_keys": len(values),
        "scale_min": tuple(min(value[axis] for value in values) for axis in range(3)),
        "scale_max": tuple(max(value[axis] for value in values) for axis in range(3)),
        "unit": all(abs(component - 1.0) <= 1e-6 for value in values for component in value),
    }


def asset_paths(asset_root):
    result = {}
    for path in asset_root.rglob("*.anim.ron"):
        result.setdefault(path.name.removesuffix(".anim.ron"), []).append(
            path.relative_to(asset_root).as_posix()
        )
    return result


def stable_id(graph, role):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"bevy-als-additive/{graph}/{role}"))


def frame_time(metadata):
    frame = metadata["reference_frame"]
    if frame == 0:
        return 0.0
    reference_rate = metadata["reference_sample_rate"]
    if reference_rate > 0.0:
        return frame / reference_rate
    reference_length = metadata["reference_sequence_length"]
    reference_frames = metadata["reference_frame_count"]
    return reference_length * frame / max(reference_frames - 1, 1)


def node_line(node_id, name, node_type, fields):
    return (
        f'        (id: "{node_id}", name: "{name}", inner: {{ "{node_type}": '
        f"({fields}) }}, should_debug: false),"
    )


def node_position(text, node_id):
    match = re.search(
        rf'"{re.escape(node_id)}"\s*:\s*\(\s*(-?[0-9.]+)\s*,\s*(-?[0-9.]+)\s*\)',
        text,
    )
    if match is None:
        return None
    return float(match.group(1)), float(match.group(2))


def occupied_positions(text):
    return {
        (float(match.group(1)), float(match.group(2)))
        for match in re.finditer(r'"[^"]+"\s*:\s*\(\s*(-?[0-9.]+)\s*,\s*(-?[0-9.]+)\s*\)', text)
    }


def reserve_position(used, position):
    x, y = position
    while (x, y) in used:
        x += 32.0
        y += 32.0
    used.add((x, y))
    return x, y


def reference_group(graph, metadata, paths, target_id):
    reference_asset = metadata["reference_asset"]
    frame = metadata["reference_frame"]
    matches = paths.get(reference_asset, [])
    if len(matches) != 1:
        raise ValueError(
            f"{graph}: reference asset {reference_asset} resolves to {len(matches)} paths"
        )
    key = f"{reference_asset}/frame-{frame}/{target_id}"
    return {
        "key": key,
        "clip_id": stable_id(graph, f"reference-clip/{key}"),
        "evaluator_id": stable_id(graph, f"reference-evaluator/{key}"),
        "constant_id": stable_id(graph, f"reference-frame/{key}"),
        "path": matches[0],
        "time": frame_time(metadata),
        "duration": metadata["reference_sequence_length"],
        "asset": reference_asset,
        "frame": frame,
        "target_id": target_id,
    }


def group_nodes(group):
    return [
        node_line(
            group["clip_id"],
            f"additive reference {group['asset']} {group['target_id'][:8]} clip",
            CLIP_NODE,
            f'clip: "{group["path"]}", override_duration: None, override_interpolation: None',
        ),
        node_line(
            group["evaluator_id"],
            f"additive reference {group['asset']} {group['target_id'][:8]} evaluator",
            "als_ext::TimePositionNode",
            f'duration_fallback: {group["duration"]!r}, loop_position: false',
        ),
        node_line(
            group["constant_id"],
            f"additive reference {group['asset']} {group['target_id'][:8]} frame",
            "als_ext::ConstantF32Node",
            f'value: {group["time"]!r}',
        ),
    ]


def group_edges(group):
    return [
        f'        NodeData("{group["evaluator_id"]}", "position"): NodeData("{group["constant_id"]}", "value"), NodeData("{group["evaluator_id"]}", "pose"): NodeData("{group["clip_id"]}", "pose"), NodeTime("{group["evaluator_id"]}", "time"): NodeTime("{group["clip_id"]}"),'
    ]


def difference_type(additive_type):
    if additive_type == LOCAL_ADDITIVE:
        return "als_ext::ReferenceLocalDifferenceNode"
    if additive_type == MESH_ADDITIVE:
        return "als_ext::ReferenceMeshDifferenceNode"
    raise ValueError(f"unsupported additive type {additive_type}")


def source_clip_metadata(source_assets, paths, graph, clip_path):
    clip = Path(clip_path).name.removesuffix(".anim.ron")
    metadata = source_assets.get(clip)
    if metadata is None or metadata["additive_type"] == "AAT_None":
        return None
    reference = source_assets.get(metadata["reference_asset"])
    if reference is None:
        raise ValueError(f"{graph}: missing source metadata for {metadata['reference_asset']}")
    if metadata["reference_type"] != "ABPT_AnimFrame":
        raise ValueError(f"{graph}: {clip} has reference type {metadata['reference_type']}")
    reference_paths = paths.get(metadata["reference_asset"], [])
    if len(reference_paths) != 1:
        raise ValueError(
            f"{graph}: reference asset {metadata['reference_asset']} resolves to {len(reference_paths)} paths"
        )
    merged = metadata | {
        "reference_sequence_length": reference["sequence_length"],
        "reference_sample_rate": reference["sample_rate"],
        "reference_frame_count": reference["frame_count"],
    }
    return clip, merged, reference_paths[0], frame_time(merged)


def patch_sequence_player_references(graph_path, graph, source_assets, paths, asset_root, write):
    text, _ = read_graph(graph_path)
    original_text = text
    rows = []

    def replace(match):
        nonlocal rows
        resolved = source_clip_metadata(source_assets, paths, graph, match.group("clip"))
        if resolved is None:
            return match.group(0)
        clip, metadata, reference_path, reference_time = resolved
        if metadata["additive_type"] not in {LOCAL_ADDITIVE, MESH_ADDITIVE}:
            raise ValueError(f"{graph}: {clip} additive type is unsupported for SourceSequencePlayer")
        gltf_paths = list(Path(asset_root).rglob(f"{clip}.gltf"))
        summary = raw_scale_summary(gltf_paths[0]) if len(gltf_paths) == 1 else {
            "scale_keys": 0,
            "scale_min": "Missing",
            "scale_max": "Missing",
            "unit": False,
        }
        expected_reference = f'Some("{reference_path}")'
        mesh_space = metadata["additive_type"] == MESH_ADDITIVE
        converted = match.group("reference") == expected_reference and abs(
            float(match.group("position")) - reference_time
        ) <= 1e-6 and (match.group("mesh") == str(mesh_space).lower())
        rows.append(
            {
                "graph": graph,
                "clip": clip,
                "metadata": metadata,
                "converted": converted or write,
                "planned": not converted,
                "reference_time": reference_time,
                "pose_consumers": 1,
                "unit": summary["unit"],
                "kind": "sequence_player",
                "requires_single_pose_consumer": False,
            }
        )
        return (
            match.group("prefix")
            + expected_reference
            + match.group("middle")
            + repr(reference_time)
            + ", additive_mesh_space: "
            + str(mesh_space).lower()
        )

    text = SEQUENCE_PLAYER_PATTERN.sub(replace, text)
    for match in SEQUENCE_EVALUATOR_PATTERN.finditer(text):
        resolved = source_clip_metadata(source_assets, paths, graph, match.group("clip"))
        if resolved is not None:
            raise ValueError(f"{graph}: {resolved[0]} additive type is unsupported for SourceSequenceEvaluator")
    if write and text != original_text:
        graph_path.write_bytes(text.encode("utf-8"))
    return rows


def referenced_animation_names(graph_root):
    names = {row["clip"] for row in graph_clip_uses(graph_root)}
    for path in sorted(graph_root.rglob("*.ron")):
        text = path.read_text(encoding="utf-8")
        names.update(
            Path(match.group("clip")).name.removesuffix(".anim.ron")
            for match in SEQUENCE_PLAYER_PATTERN.finditer(text)
        )
        names.update(
            Path(match.group("clip")).name.removesuffix(".anim.ron")
            for match in SEQUENCE_EVALUATOR_PATTERN.finditer(text)
        )
    return names


def patch_graph(graph_path, graph, rows, paths, write):
    text, newline = read_graph(graph_path)
    original_text = text
    added_nodes = []
    added_edges = []
    added_positions = {}
    used_positions = occupied_positions(text)
    groups = {}
    statuses = []
    for row in rows:
        metadata = row["metadata"]
        if metadata["reference_type"] != "ABPT_AnimFrame":
            raise ValueError(
                f"{graph}: {row['clip']} has reference type {metadata['reference_type']}"
            )
        group = reference_group(graph, metadata, paths, row["node_id"])
        groups.setdefault(group["key"], group)
        difference_id = stable_id(graph, f"difference/{row['node_id']}")
        converted = f'id: "{difference_id}"' in text
        planned = False
        if not converted:
            planned = True
            position = node_position(original_text, row["node_id"])
            if position is None:
                raise ValueError(f"{graph}: {row['node']} has no editor position")
            source_pose = f'NodeData("{row["node_id"]}", "pose")'
            source_time = f'NodeTime("{row["node_id"]}")'
            if source_pose not in text or source_time not in text:
                raise ValueError(f"{graph}: {row['node']} has no pose/time consumer")
            text = text.replace(source_pose, f'NodeData("{difference_id}", "pose")')
            text = text.replace(source_time, f'NodeTime("{difference_id}")')
            added_nodes.append(
                node_line(
                    difference_id,
                    f"additive difference {row['node']}",
                    difference_type(metadata["additive_type"]),
                    "",
                )
            )
            added_positions[difference_id] = reserve_position(
                used_positions, (position[0] + 96.0, position[1])
            )
            if "positions" not in group:
                group["positions"] = {
                    group["clip_id"]: reserve_position(
                        used_positions, (position[0] - 288.0, position[1])
                    ),
                    group["evaluator_id"]: reserve_position(
                        used_positions, (position[0] - 192.0, position[1])
                    ),
                    group["constant_id"]: reserve_position(
                        used_positions, (position[0] - 288.0, position[1] + 48.0)
                    ),
                }
                added_positions.update(group["positions"])
            added_edges.append(
                f'        NodeData("{difference_id}", "reference_pose"): NodeData("{group["evaluator_id"]}", "pose"), NodeTime("{difference_id}", "reference_time"): NodeTime("{group["evaluator_id"]}"), NodeData("{difference_id}", "target_pose"): NodeData("{row["node_id"]}", "pose"), NodeTime("{difference_id}", "target_time"): NodeTime("{row["node_id"]}"),'
            )
            converted = write
        pose_consumers = text.count(f'NodeData("{difference_id}", "pose")')
        statuses.append(
            row
            | {
                "converted": converted,
                "planned": planned,
                "reference_time": group["time"],
                "pose_consumers": pose_consumers,
            }
        )
    if added_nodes:
        for group in groups.values():
            if f'id: "{group["clip_id"]}"' not in original_text:
                added_nodes.extend(group_nodes(group))
                added_edges.extend(group_edges(group))
        nodes_marker = f"{newline}    ],{newline}    edges_inverted:"
        edges_marker = f"{newline}    }},{newline}    io_spec:"
        if nodes_marker not in text or edges_marker not in text:
            raise ValueError(f"{graph}: graph layout markers missing")
        nodes_index = text.index(nodes_marker)
        node_separator = "" if text[:nodes_index].rstrip().endswith(",") else ","
        text = text.replace(
            nodes_marker,
            node_separator + f"{newline}" + newline.join(added_nodes) + nodes_marker,
            1,
        )
        text = text.replace(edges_marker, f"{newline}" + newline.join(added_edges) + edges_marker, 1)
        positions_marker = "node_positions: {"
        if positions_marker not in text:
            raise ValueError(f"{graph}: editor positions are missing")
        positions = ", ".join(
            f'"{node_id}": ({x:.3f}, {y:.3f})'
            for node_id, (x, y) in sorted(added_positions.items())
        )
        text = text.replace(positions_marker, positions_marker + " " + positions + ",", 1)
        if write:
            graph_path.write_bytes(text.encode("utf-8"))
    return statuses


def markdown(rows):
    lines = [
        "# ALS Additive Reference Delta Audit",
        "",
        "This report is generated from original AnimSequence metadata and the checked-in AB_Als graph tree.",
        "",
        "| Graph | Clip node | Asset | Additive type | Reference asset | Reference frame | Reference time | Raw scale keys | Unit scale | Pose consumers | Delta conversion | Planned conversion |",
        "| --- | --- | --- | --- | --- | ---: | ---: | ---: | --- | ---: | --- | --- |",
    ]
    for row in rows:
        metadata = row["metadata"]
        lines.append(
            f"| `{row['graph']}` | {row['node']} | {row['clip']} | {metadata['additive_type']} | {metadata['reference_asset']} | {metadata['reference_frame']} | {row['reference_time']:.9f} | {row['scale_keys']} | {row['unit']} | {row['pose_consumers']} | {row['converted']} | {row['planned']} |"
        )
    converted = sum(row["converted"] for row in rows)
    unit = sum(row["unit"] for row in rows)
    single_pose_consumers = sum(row["pose_consumers"] == 1 for row in rows)
    lines.extend(
        [
            "",
            f"## Referenced Additive Clips: {len(rows)}",
            f"## Converted Inputs: {converted}",
            f"## Unit Raw Scale Inputs: {unit}",
            f"## Single Pose Consumers: {single_pose_consumers}",
            "",
        ]
    )
    return "\n".join(lines)


def run_self_test():
    def native_sequence(path, additive_type="AAT_None", reference="None"):
        return {
            "format": "ue_asset_source_v1",
            "asset_path": path,
            "asset_class": "/Script/Engine.AnimSequence",
            "asset_properties": [
                {"name": "AdditiveAnimType", "exported_text": additive_type},
                {"name": "RefPoseType", "exported_text": "ABPT_AnimFrame"},
                {"name": "RefPoseSeq", "exported_text": reference},
                {"name": "RefFrameIndex", "exported_text": "0"},
                {"name": "SequenceLength", "exported_text": "1.000000"},
                {"name": "SamplingFrameRate", "exported_text": "(Numerator=30,Denominator=1)"},
                {"name": "NumFrames", "exported_text": "31"},
            ],
        }

    base_path = "/ALS/ALS/Base/A_Base.A_Base"
    additive_path = "/ALS/ALS/Animations/A_Add.A_Add"
    additive = native_sequence(
        additive_path,
        MESH_ADDITIVE,
        "\"/Script/Engine.AnimSequence'/ALS/ALS/Base/A_Base.A_Base'\"",
    )
    assert native_animation_metadata(additive, additive_path)["reference_asset"] == "A_Base"
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "ALS"
        add_file = root / "Animations" / "A_Add.uasset"
        base_file = root / "Base" / "A_Base.uasset"
        add_file.parent.mkdir(parents=True)
        base_file.parent.mkdir(parents=True)
        add_file.write_bytes(b"fixture")
        base_file.write_bytes(b"fixture")
        original_raw = globals()["raw_animation_metadata"]
        original_run = source_resolver.subprocess.run
        try:
            def unreadable_raw(_):
                raise RuntimeError("cooked fixture")

            reader_calls = []

            def fake_reader(command, **_kwargs):
                source_path = command[command.index("-Asset") + 1]
                output = Path(command[command.index("-Output") + 1])
                output.write_text(
                    json.dumps(
                        {
                            "format": "ue_asset_source_v1",
                            "asset_path": source_path,
                            "asset_class": "/Script/Engine.AnimSequence",
                            "asset_properties": {
                                additive_path: additive,
                                base_path: native_sequence(base_path),
                            }[source_path]["asset_properties"],
                        }
                    ),
                    encoding="utf-8",
                )
                reader_calls.append(source_path)
                return type("ReaderResult", (), {"returncode": 0})()

            globals()["raw_animation_metadata"] = unreadable_raw
            source_resolver.native_source_asset_cached.cache_clear()
            source_resolver.subprocess.run = fake_reader
            source_resolver.native_source_asset(additive_path)
            assets = source_animation_assets(root, {"A_Add"}, "/ALS/ALS")
            assert set(assets) == {"A_Add", "A_Base"}
            assert assets["A_Add"]["reference_asset"] == "A_Base"
            assert reader_calls == [additive_path, base_path]
        finally:
            globals()["raw_animation_metadata"] = original_raw
            source_resolver.subprocess.run = original_run
            source_resolver.native_source_asset_cached.cache_clear()
    return {
        "native_additive_metadata_preserved": "PASS",
        "cooked_additive_reference_fallback_reuses_native_source_records": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--graph-root", type=Path, default=Path("assets/graphs/ab_als"))
    parser.add_argument("--asset-root", type=Path, default=Path("assets"))
    parser.add_argument("--report", type=Path, default=Path("docs/bf100-additive-assets.md"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), indent=2))
        return
    if args.source_root is None:
        parser.error("--source-root is required")
    assets = source_animation_assets(args.source_root)
    paths = asset_paths(args.asset_root)
    grouped = {}
    for row in graph_clip_uses(args.graph_root):
        metadata = assets.get(row["clip"])
        if metadata is None or metadata["additive_type"] == "AAT_None":
            continue
        reference = assets.get(metadata["reference_asset"])
        if reference is None:
            raise ValueError(f"{row['clip']}: missing source metadata for {metadata['reference_asset']}")
        gltf_paths = list(args.asset_root.rglob(f"{row['clip']}.gltf"))
        summary = raw_scale_summary(gltf_paths[0]) if len(gltf_paths) == 1 else {
            "scale_keys": 0,
            "scale_min": "Missing",
            "scale_max": "Missing",
            "unit": False,
        }
        merged = metadata | {
            "reference_sequence_length": reference["sequence_length"],
            "reference_sample_rate": reference["sample_rate"],
            "reference_frame_count": reference["frame_count"],
        }
        grouped.setdefault(row["path"], []).append(row | {"metadata": merged} | summary)
    rows = []
    for path, graph_rows in grouped.items():
        rows.extend(patch_graph(path, graph_rows[0]["graph"], graph_rows, paths, args.write))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(markdown(rows), encoding="utf-8", newline="\n")
    valid = bool(rows) and all(
        row["converted"] and row["unit"] and row["pose_consumers"] == 1 for row in rows
    )
    print(
        json.dumps(
            {
                "report": args.report.as_posix(),
                "additive_clip_uses": len(rows),
                "converted_inputs": sum(row["converted"] for row in rows),
                "unit_raw_scale_inputs": sum(row["unit"] for row in rows),
                "single_pose_consumers": sum(row["pose_consumers"] == 1 for row in rows),
                "pass": valid,
            },
            indent=2,
        )
    )
    if args.check and not valid:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
