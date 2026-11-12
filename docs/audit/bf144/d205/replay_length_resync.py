import gzip
import json
import math
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    root = Path(__file__).parent
    request = json.loads((root / "resync.request.json").read_bytes())
    path = root / "resync.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)["sync_group_clock_probe"]
    players = []
    for requested, observed in zip(request["players"], native["players"], strict=True):
        assert requested["asset"] == observed["asset_path"] and not observed.get("markers")
        assert requested["start"] == 0.0
        players.append({"asset": requested["asset"], "duration": observed["duration"],
                        "rate": f32(requested["rate"] * observed["rate_scale"]), "looped": requested["looping"]})

    cases = []
    local_time_ablation = []
    wrapped_ratio_ablation = []
    for requested, observed in zip(request["samples"], native["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        frames = []
        for step, frame in zip(requested["steps"], observed["frames"], strict=True):
            index, weight = next((index, f32(weight)) for index, weight in enumerate(step["weights"]) if weight is not None)
            assert frame["order"] == [index] and frame["leader"] == index
            player = players[index]
            duration, rate = player["duration"], player["rate"]
            delta = f32(step["delta_seconds"])
            prior = frames[-1] if frames else None
            resync = bool(prior and step["inertial"] and prior["weight"] >= weight)
            start = f32(prior["ratio"] * duration) if resync else 0.0
            time = f32(start + f32(delta * rate))
            if time > duration:
                time = f32(math.fmod(time, duration)) if player["looped"] else duration
            clock = frame["players"][index]
            assert [start, time] == [clock["previous"], clock["position"]], name
            assert f32(time / duration) == frame["ratio"], name
            if resync and prior["player"] == index and start != 0.0:
                local_time_ablation.append(name)
            if resync and players[prior["player"]]["looped"] and prior["ratio"] == 1.0:
                wrapped_ratio_ablation.append(name)
            frames.append({"player": index, "weight": weight, "delta": delta, "inertial": step["inertial"],
                           "clock": [start, time], "ratio": frame["ratio"]})
        cases.append({"name": name, "frames": frames})
    assert local_time_ablation == ["same_identity_early", "same_identity_equal", "loop_endpoint"]
    assert wrapped_ratio_ablation == ["loop_endpoint"]
    print(json.dumps({"players": players, "cases": cases, "local_time_ablation": local_time_ablation,
                      "wrapped_ratio_ablation": wrapped_ratio_ablation}, indent=2))


if __name__ == "__main__":
    main()
