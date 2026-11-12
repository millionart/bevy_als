import argparse
import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d224d"))
sys.path.insert(0, str(HERE.parent / "d228"))
from replay_pose_consumers import components, project, read_capture
from replay_source_frame_time import source_rotations
from compile_als_event_tracks import f32


def compare(before, after, normalize, apply=True):
    counts = Counter()
    witness = None
    for start, end in zip(before["bones"], after["bones"]):
        assert start["name"] == end["name"]
        old = components(start["local_transform"])
        new = components(end["local_transform"])
        expected = normalize(old[0]) if apply else old[0]
        assert expected == new[0] and old[1:] == new[1:], (start["name"], expected, new[0])
        counts["exact"] += 1
        counts["outer_double_change"] += old[0] != new[0]
        projected_change = project(old)[0] != project(new)[0]
        counts["outer_f32_change"] += projected_change
        if projected_change and witness is None:
            witness = {"bone": start["name"], "before": project(old)[0], "after": project(new)[0]}
    return counts, witness


def replay(work):
    normalize, _ = source_rotations(work)
    native = read_capture(HERE / "blend.native.json.gz")["sync_group_clock_probe"]
    blend = Counter()
    types = Counter()
    blend_witness = None
    for case in native["samples"]:
        for frame in case["frames"]:
            for player in frame["players"]:
                if player["pose"] is None:
                    continue
                count = len(player["samples"])
                assert count in (1, 2)
                types[count] += 1
                if count == 1:
                    assert player["samples"][0]["weight"] == 1.0
                    assert player["samples"][0]["pose"]["bones"] == player["blend_inner_pose"]["bones"]
                values, witness = compare(player["blend_inner_pose"], player["pose"], normalize)
                blend.update(values)
                if blend_witness is None and witness is not None and count == 2:
                    blend_witness = witness
    assert types[1] > 0 and types[2] > 0 and blend_witness, (types, blend)
    head = read_capture(HERE / "head.native.json.gz")["mesh_additive_probe"]
    additive = Counter()
    head_witness = None
    for frame in head["samples"]:
        source = frame["api_pose"]
        final = frame["pose"]
        assert source["curves"] == final["curves"]
        active = frame["alpha"] > f32(1e-5)
        counts, witness = compare(source, final, normalize, active)
        additive.update(counts)
        if active and head_witness is None:
            head_witness = witness
    assert additive["outer_double_change"] > 0 and head_witness, additive
    rotation = (-0.055010028183460236, 0.5095681548118591, -0.4557597041130066, 0.8817023634910583)
    once = normalize(rotation)
    twice = normalize(once)
    zero = (0.0,) * 3
    target_witness = {"input": project((rotation, zero, zero))[0],
                      "once": project((once, zero, zero))[0],
                      "twice": project((twice, zero, zero))[0]}
    assert target_witness["once"] != target_witness["twice"]
    return {"result": "PASS_SOURCE_POSE_NORMALIZATION_STAGES", "blend_sample_counts": dict(types),
            "blend": dict(blend), "blend_witness": blend_witness, "mesh_additive": dict(additive),
            "mesh_additive_witness": head_witness, "f32_input_witness": target_witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d231")
    print(json.dumps(replay(parser.parse_args().work), indent=2))
