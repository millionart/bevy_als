import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d228"))
from replay_source_frame_time import f32, frame_time, read_capture, sample_curve


def replay():
    curves = json.loads((ROOT / "assets/ALSRuntime/Animations/als_raw_sequencer_curves.json").read_bytes())
    sequence = next(row for row in curves if row["animation"] == "A_Als_Crouch_Walk_Right_Backward_0")
    rate = sequence["frame_rate"]
    epsilon = f32(1e-8)
    results = {}
    counts = Counter()
    for name in ("cold", "history"):
        native = read_capture(HERE / f"{name}.native.json.gz")
        previous = {}
        cold_zero = {curve["name"].lower(): 0.0 for curve in sequence["curves"]}
        comparisons = []
        for frame in native["sequence_pose_probe"]["frames"]:
            assert frame["pose_evaluation_mode"] == "raw_model"
            source_time = frame_time(frame["time"], rate)
            requested = {curve["name"].lower(): sample_curve(curve, source_time, rate)
                         for curve in sequence["curves"]}
            expected = {key.lower(): value for key, value in frame["curves"].items()}
            assert requested.keys() == expected.keys()
            for key, value in requested.items():
                if key not in previous or abs(f32(previous[key] - value)) > epsilon:
                    previous[key] = value
                else:
                    counts["held_writes"] += previous[key] != value
                if abs(f32(cold_zero[key] - value)) > epsilon:
                    cold_zero[key] = value
                counts["stateless_ablation"] += value != expected[key]
                counts["initial_zero_ablation"] += cold_zero[key] != expected[key]
                counts["native_values"] += 1
            assert previous == expected, (name, frame["time"], previous, expected)
            comparisons.append({"time": frame["time"], "requested": requested["feetcrossing"],
                                "published": previous["feetcrossing"]})
        results[name] = comparisons
    history = results["history"]
    assert history[1]["time"] == history[3]["time"]
    assert history[1]["requested"] == history[3]["requested"] == history[3]["published"]
    assert history[0]["published"] == history[1]["published"] != history[3]["published"]
    assert all(counts[key] > 0 for key in ("stateless_ablation", "initial_zero_ablation", "held_writes"))
    return {"result": "PASS_RAW_FK_CURVE_WRITEBACK", "source_curve": "FeetCrossing",
            "contexts": results, **counts}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
