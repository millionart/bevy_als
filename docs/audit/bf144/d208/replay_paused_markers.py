import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    root = Path(__file__).parent
    path = root / "marker.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)["sync_group_clock_probe"]
    request = json.loads((root / "marker.request.json").read_bytes())
    players = []
    for requested, observed in zip(request["players"], native["players"], strict=True):
        assert requested["asset"] == observed["asset_path"] and requested["looping"]
        assert [marker["name"] for marker in observed["markers"]] == ["Left", "Right"]
        players.append({"duration": observed["duration"], "markers": observed["markers"],
                        "rate_scale": observed["rate_scale"], "start": f32(requested["start"])})
    cases = []
    paused_frames = 0
    paused_attempts = 0
    restart_frames = 0
    stale_phase_start = None
    for requested, observed in zip(request["samples"], native["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        positions = [player["start"] for player in players]
        frames = []
        for request_frame, frame in zip(requested["steps"], observed["frames"], strict=True):
            delta = f32(request_frame["delta_seconds"])
            weights = request_frame["weights"]
            rates = request_frame.get("rates", [row["rate"] for row in request["players"]])
            rates = [f32(rate * player["rate_scale"]) for rate, player in zip(rates, players, strict=True)]
            order = frame["order"]
            leader = frame["leader"]
            attempted = order[:order.index(leader) + 1]
            clocks = [[frame["players"][index]["previous"], frame["players"][index]["position"]] for index in attempted]
            start = frame["marker_start"] if frame["marker_start"]["valid"] else None
            end = frame["marker_end"] if frame["marker_end"]["valid"] else None
            all_paused = all(f32(delta * rates[index]) == 0.0 for index in order)
            for index, clock in zip(attempted, clocks, strict=True):
                if f32(delta * rates[index]) == 0.0:
                    assert clock == [positions[index], positions[index]], (name, index)
                    paused_attempts += 1
            if all_paused:
                assert leader == order[-1] and attempted == order
                assert start is None and end is None
                paused_frames += 1
            else:
                assert start is not None and end is not None
            after_pause = bool(frames and frames[-1]["end"] is None and not all_paused)
            if after_pause:
                restart_frames += 1
                assert clocks[-1] == [positions[leader], f32(positions[leader] + f32(delta * rates[leader]))]
                if name == "zero_then_new_leader" and stale_phase_start is None:
                    canonical = players[0]["markers"]
                    previous = frames[-1]["inputs"][frames[-1]["leader"]]
                    phase = f32(f32(previous - canonical[0]["time"]) / f32(canonical[1]["time"] - canonical[0]["time"]))
                    target = players[leader]["markers"]
                    stale = f32(target[0]["time"] + f32(phase * f32(target[1]["time"] - target[0]["time"])))
                    assert abs(stale - positions[leader]) > 0.5
                    stale_phase_start = [stale, positions[leader]]
            frames.append({"delta": delta, "weights": weights, "rates": rates, "inputs": positions.copy(),
                           "leader": leader, "attempted": attempted, "clocks": clocks, "start": start, "end": end,
                           "ratios": [frame["previous_ratio"], frame["ratio"]], "after_pause": after_pause})
            positions = [clock["position"] for clock in frame["players"]]
        cases.append({"name": name, "frames": frames})
    assert paused_frames == 5 and paused_attempts == 12 and restart_frames == 5 and stale_phase_start
    print(json.dumps({"players": players, "cases": cases, "paused_frames": paused_frames, "paused_attempts": paused_attempts,
                      "restart_frames": restart_frames, "stale_phase_start_ablation": stale_phase_start}, indent=2))


if __name__ == "__main__":
    main()
