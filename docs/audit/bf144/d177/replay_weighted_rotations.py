import gzip
import json
import math
import sys
from pathlib import Path


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


def runtime(q):
    x, y, z, w = q
    return -x, -z, -y, w


directory = Path(__file__).resolve().parent
sys.path.insert(0, str(directory.parent / "d123a"))
from replay_blend import add_shortest, distance, dot, f32, normalize, quat, scale


def slerp(a, b, weight):
    a, b = normalize(a), normalize(b)
    cosine = dot(a, b)
    if cosine < 0.0:
        b = scale(b, -1.0)
        cosine = -cosine
    if cosine > 0.9995:
        return normalize(add_shortest(scale(a, 1.0 - weight), scale(b, weight)))
    angle = math.acos(min(1.0, cosine))
    return add_shortest(scale(a, math.sin((1.0 - weight) * angle) / math.sin(angle)),
                        scale(b, math.sin(weight * angle) / math.sin(angle)))


cases = []
checked = 0
old_failures = 0
for stance in ("standing", "crouching"):
    native = load(directory / f"{stance}-turn.native.json")["slot_probe"]
    request = load(directory / f"{stance}-turn.request.json")
    references = {0.25: native["frames"][0]["bones"], 0.8: native["frames"][1]["bones"]}
    worst = None
    for definition, row in zip(request["steps"][2:], native["frames"][2:], strict=True):
        positions = row["evaluation_positions"]
        weights = list(map(f32, row["evaluation_weights"]))
        assert positions == definition["evaluation_positions"] and row["evaluation_weights"] == definition["evaluation_weights"]
        assert row["source_weight"] == 0.0 and row["slot_weight"] == row["total_weight"] == 1.0
        for a, b, result in zip(references[positions[0]], references[positions[1]], row["bones"], strict=True):
            assert a["name"] == b["name"] == result["name"]
            left, right = quat(a), quat(b)
            raw = add_shortest(scale(left, weights[0]), scale(right, weights[1]))
            actual = normalize(quat(result))
            assert distance(normalize(raw), actual) < 1e-12
            error = distance(actual, slerp(left, right, weights[1]))
            checked += 1
            old_failures += error > 1e-6
            if worst is None or error > worst["old_direction_error"]:
                worst = {"stance": stance, "bone": a["name"], "base": runtime(left), "target": runtime(right),
                         "weights": weights, "raw_rotation": runtime(raw), "rotation": runtime(quat(result)),
                         "old_direction_error": error}
    assert worst is not None and worst["old_direction_error"] > 1e-5
    cases.append(worst)

assert checked == 632 and old_failures > 0
print(json.dumps({"native_bone_rotations": checked, "old_slerp_failures": old_failures, "cases": cases}, indent=2))
