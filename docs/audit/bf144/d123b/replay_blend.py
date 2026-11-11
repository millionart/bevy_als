from functools import reduce
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / "d123a"))
from replay_blend import add_shortest, distance, normalize, quat, scale


def verify(path):
    doc = json.loads(path.read_text())
    results = {count:{"bones":0, "early_normalization_failures":0,
                      "max_native_orientation_error":0.0, "max_early_error":0.0}
               for count in (2, 3)}
    for sample in doc["blend_list_probe"]["samples"]:
        for frame in sample["frames"]:
            inputs = [(pose, weight) for pose, weight in zip(frame["input_poses"], frame["weights"]) if weight > 0]
            count = len(inputs) - 1
            if count not in results:
                continue
            result = results[count]
            for index, bone in enumerate(frame["pose"]["bones"]):
                scaled = [scale(quat(pose["bones"][index]), weight) for pose, weight in inputs]
                accumulated = reduce(add_shortest, scaled[:-1])
                direction = normalize(quat(bone))
                expected = normalize(add_shortest(accumulated, scaled[-1]))
                early = scale(normalize(accumulated), sum(weight for _, weight in inputs[:-1]))
                difference = distance(direction, normalize(add_shortest(early, scaled[-1])))
                result["bones"] += 1
                result["max_native_orientation_error"] = max(result["max_native_orientation_error"], distance(direction, expected))
                result["max_early_error"] = max(result["max_early_error"], difference)
                result["early_normalization_failures"] += difference > 1e-6
    for count, result in results.items():
        assert result["max_native_orientation_error"] < 1e-12, (count, result)
        assert result["bones"] > 0 and result["early_normalization_failures"] == result["bones"], (count, result)
    return results


if __name__ == "__main__":
    print(json.dumps(verify(ROOT / "blend.native.json"), indent=2))
