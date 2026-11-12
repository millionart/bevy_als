import argparse
import json
import re
import sys
from collections import Counter
from functools import lru_cache
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232d"))
from replay_source_additive import PACK, ROOT, bones, clip_data, components, difference, project, read_capture, source_pose, source_rotations


REFERENCE_ASSET = "ALSRuntime/Animations/als_raw_additive_references.json"


def capture(stem):
    path = HERE / f"{stem}.native.json"
    return json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))


def bindings(assets, prepared):
    @lru_cache(maxsize=None)
    def clip_kind(clip):
        text = (assets / clip).read_text()
        source = re.search(r'\bpath:\s*"([^"]+)".*?\banimation_name:\s*"([^"]+)"', text, re.S)
        return prepared[source.groups()]["additive_type"]

    totals = Counter()
    for path in sorted((assets / "ALS/Character").rglob("*.animgraph.ron")):
        graph = path.read_text()
        nodes = {match[1]: line for line in graph.splitlines()
                 if (match := re.match(r'\s*\(id: "([^"]+)".*inner: ', line))}
        for node_id, line in nodes.items():
            sequence = re.search(r'SourceSequence(?:Player|Evaluator)Node": \(clip: "([^"]+)".*?additive_reference: Some\("([^"]+)"\)', line)
            if sequence:
                assert clip_kind(sequence[1]) in (1, 2), path
                totals["baked_sequence_references"] += 1
            wrapper = re.search(r'"als_ext::Reference(Local|Mesh)DifferenceNode"', line)
            if wrapper:
                target = re.search(rf'NodeData\("{node_id}", "target_pose"\): NodeData\("([^"]+)"', graph)[1]
                clip = re.search(r'clip: "([^"]+)"', nodes[target])[1]
                assert clip_kind(clip) == {"Local": 1, "Mesh": 2}[wrapper[1]], path
                outputs = re.findall(rf'NodeData\("([^"]+)", "[^"]+"\): NodeData\("{node_id}", "pose"\)', graph)
                assert outputs and all("NormalizedBlendSpaceNode" in nodes[output] for output in outputs), path
                totals["baked_blendspace_wrappers"] += 1
    return totals


def replay(work):
    metadata, keys = clip_data(ROOT / PACK)
    parents = [bone["parent_index"] for bone in metadata["bones"]]
    prepared = {(row["gltf"], row["animation"]): row for row in metadata["sequences"]}
    generated = json.loads((ROOT / "assets" / REFERENCE_ASSET).read_bytes())
    normalize, _ = source_rotations(work)
    counts = bindings(ROOT / "assets", prepared)
    assert counts["baked_sequence_references"] == 51
    assert counts["baked_blendspace_wrappers"] == 23
    for stem, target, mesh in (("lean", "lean", None), ("stand", "look", parents)):
        native = capture(stem)
        sequence = next(row for row in prepared.values() if row["asset"] == native["asset_path"])
        row = next(row for row in generated if row["gltf"] == sequence["gltf"] and row["animation"] == sequence["animation"])
        frame, = native["sequence_pose_probe"]["frames"]
        assert row["time"] == frame["time"] == 0.0
        assert row["bones"] == [bone["local_transform"] for bone in frame["bones"]]
        reference = [components(bone) for bone in row["bones"]]
        ordinary = source_pose(sequence, metadata, keys, 0.0, normalize)
        counts["raw_reference_bones"] += len(reference)
        counts[f"{stem}_ordinary_reference_mismatches"] = sum(a != b for a, b in zip(reference, ordinary, strict=True))
        absolute = read_capture(HERE.parent / f"d232d/{target}-absolute.native.json.gz")["sequence_pose_probe"]["frames"]
        additive = read_capture(HERE.parent / f"d232d/{target}-delta.native.json.gz")["sequence_pose_probe"]["frames"]
        for a, b in zip(absolute, additive, strict=True):
            assert a["time"] == b["time"]
            desired = difference(reference, bones(a), mesh, normalize)
            assert desired == bones(b)
            control = difference(ordinary, bones(a), mesh, normalize)
            counts["native_additive_bones"] += len(desired)
            counts[f"{stem}_ordinary_reference_f32_delta_mismatches"] += sum(
                project(a) != project(b) for a, b in zip(desired, control, strict=True))
    assert len(generated) == 2 and counts["raw_reference_bones"] == 158
    assert counts["lean_ordinary_reference_f32_delta_mismatches"] and counts["stand_ordinary_reference_f32_delta_mismatches"]
    return {"result": "PASS_RAW_ADDITIVE_REFERENCE", **counts}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d235")
    print(json.dumps(replay(parser.parse_args().work), indent=2))
