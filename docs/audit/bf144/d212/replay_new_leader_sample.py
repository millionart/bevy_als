import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def read(root, name):
    path = root / name
    if path.is_file():
        return json.loads(path.read_bytes())
    return json.loads(gzip.decompress(path.with_suffix(".json.gz").read_bytes()))


def main():
    root = Path(__file__).parent
    request = read(root, "leader.request.json")
    native = read(root, "leader.native.json")["sync_group_clock_probe"]
    sequences = [read(root, f"{name}_walk.native.json") for name in ("right", "forward")]
    cases = []
    late_local = {}
    for requested, observed in zip(request["samples"], native["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        first, second = observed["frames"][:2]
        index = first["leader"]
        assert second["leader"] == index and first["order"] == second["order"] == [index]
        old = first["players"][index]["samples"]
        new = second["players"][index]["samples"]
        assert new[0]["sample_index"] == 0
        previous_sample = next((sample for sample in old if sample["sample_index"] == 0), None)
        sequence = sequences[index]
        timing = sequence["sequence_timing"]
        assert sequence["asset_path"] == new[0]["animation"]
        markers = {marker["name"]: marker["time"] for marker in timing["markers"]}
        anchor = first["marker_end"]
        assert anchor["valid"] and anchor["previous"] == "Left" and anchor["next"] == "Right"
        span = f32(markers[anchor["next"]] - markers[anchor["previous"]])
        mapped = f32(markers[anchor["previous"]] + f32(anchor["alpha"] * span))
        delta = f32(requested["steps"][1]["delta_seconds"])
        rate = f32(timing["rate_scale"] * new[0]["play_rate"])
        duration = timing["duration"]
        local = f32(f32(request["players"][index]["start"]) * duration) if previous_sample is None else previous_sample["time"]
        previous = mapped if previous_sample is None else local
        clock = [new[0]["previous_time"], new[0]["time"]]
        assert clock == [previous, f32(previous + f32(delta * rate))], name
        if previous_sample is None:
            assert old[0]["sample_index"] == 1 and len(old) == 1
            assert requested["steps"][0]["delta_seconds"] <= 1e-4
            assert previous != local
            late_local[name] = [local, previous]
        else:
            if name.endswith("existing_follower"):
                assert old[0]["sample_index"] == 1 and len(old) == 2
            else:
                assert name.endswith("above") and old[0]["sample_index"] == 0
        cases.append({"name": name, "duration": duration, "markers": timing["markers"], "anchor": anchor,
                      "local": local, "local_record": previous_sample is not None,
                      "delta": delta, "rate": rate, "clock": clock})
    assert len(cases) == 8 and len(late_local) == 4
    print(json.dumps({"cases": cases, "native_frames": sum(len(case["frames"]) for case in native["samples"]),
                      "local_start_ablation": late_local}, indent=2))


if __name__ == "__main__":
    main()
