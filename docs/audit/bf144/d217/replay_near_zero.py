import gzip
import json
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def read(path):
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def clocks(player):
    return [[sample["sample_index"], sample["previous_time"], sample["time"]] for sample in player["samples"]]


def main():
    request = read(AUDIT / "near_zero.request.json")
    source = read(AUDIT / "near_zero.native.json")["sync_group_clock_probe"]
    timing = [
        [read(AUDIT.parent / f"d212/{name}_walk.native.json"), read(AUDIT / f"{name}_pose.native.json")]
        for name in ("forward", "right")
    ]
    cases = []
    counts = {"cold_leader_publications": 0, "warm_leader_holds": 0, "zero_follower_holds": 0, "invalid_groups": 0}
    ablations = {"first_leader": 0, "publish_warm_end": 0, "previous_as_current": 0, "sync_zero_follower": 0}
    for requested, observed in zip(request["samples"], source["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        boundary = 4 if name.startswith("warm") else 0
        frames = []
        for index, (step, frame) in enumerate(zip(requested["steps"], observed["frames"], strict=True)):
            weights = step["weights"]
            delta = f32(step["delta_seconds"])
            active = frame["order"]
            leader = frame["leader"]
            assert leader in active
            valid = frame["marker_end"]["valid"]
            holds = set()
            cold = set()
            if index == boundary:
                prior = observed["frames"][index - 1] if index else None
                warm = set() if prior is None else {player for player in active if prior["players"][player]["ticked"]}
                near_zero = abs(delta) <= f32(1e-8)
                candidate = next((player for player in active if not near_zero or player not in warm), None)
                assert leader == (active[-1] if candidate is None else candidate), name
                assert valid == (candidate is not None), name
                if candidate is None:
                    counts["invalid_groups"] += 1
                if near_zero:
                    ablations["first_leader"] += leader != active[0]
                    ablations["publish_warm_end"] += not valid
                    for player in active[:active.index(leader) + 1]:
                        if player in warm:
                            assert clocks(frame["players"][player]) == clocks(prior["players"][player])
                            holds.add(player)
                            counts["warm_leader_holds"] += 1
                        else:
                            sample = frame["players"][player]["samples"][0]
                            duration = timing[player][sample["sample_index"]]["sequence_timing"]["duration"]
                            normalized = f32(request["players"][player]["start"])
                            assert sample["previous_time"] == 0 and sample["time"] == f32(normalized * duration)
                            assert frame["previous_ratio"] == normalized and valid
                            cold.add(player)
                            counts["cold_leader_publications"] += 1
                if delta == 0.0 and valid:
                    for player in active[active.index(leader) + 1:]:
                        if player in warm:
                            assert clocks(frame["players"][player]) == clocks(prior["players"][player])
                            holds.add(player)
                        else:
                            assert all(previous == time == 0.0 for _, previous, time in clocks(frame["players"][player]))
                            cold.add(player)
                        counts["zero_follower_holds"] += 1
                        ablations["sync_zero_follower"] += 1
                for player in holds:
                    ablations["previous_as_current"] += sum(
                        previous != time for _, previous, time in clocks(frame["players"][player])
                    )
            fixture_players = []
            for player, record in enumerate(frame["players"]):
                if not record["ticked"]:
                    fixture_players.append(None)
                    continue
                for sample in record["samples"]:
                    assert timing[player][sample["sample_index"]]["asset_path"] == sample["animation"]
                fixture_players.append({
                    "position": [record["filtered_position"][axis] for axis in "xy"],
                    "clocks": clocks(record), "normalized": record["position"],
                    "hold": player in holds, "cold": player in cold,
                })
            frames.append({"delta": delta, "weights": weights, "players": fixture_players, "leader": leader, "valid": valid})
        cases.append({"name": name, "frames": frames})
    assert counts == {"cold_leader_publications": 11, "warm_leader_holds": 9, "zero_follower_holds": 2, "invalid_groups": 3}
    assert ablations["first_leader"] == 6 and ablations["publish_warm_end"] == 3
    assert ablations["previous_as_current"] > 0 and ablations["sync_zero_follower"] == 2
    print(json.dumps({"timing": [[clip["sequence_timing"] for clip in space] for space in timing],
                      "cases": cases, "native_frames": sum(len(case["frames"]) for case in cases),
                      "boundaries": counts, "ablations": ablations}, indent=2))


if __name__ == "__main__":
    main()
