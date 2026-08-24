import argparse
import json
import math
import os
import struct
import sys

CLIPS = [
    ("A_Als_Mantle_High", "Actions/Mantle/A_Als_Mantle_High", None, (1.8, 2.2), 80.0 / 30.0),
    ("A_Als_Mantle_Low_Left", "Actions/Mantle/A_Als_Mantle_Low_Left", None, (0.3, 1.1), 45.0 / 30.0),
    ("A_Als_Mantle_Low_Right", "Actions/Mantle/A_Als_Mantle_Low_Right", None, (0.3, 1.1), 45.0 / 30.0),
    ("A_Als_Roll", "Actions/Roll/A_Als_Roll", (2.0, 4.0), None, 45.0 / 30.0),
]


def read_accessor(doc, blob, acc_idx):
    acc = doc["accessors"][acc_idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    ncomp = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    base = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    out = []
    for i in range(acc["count"]):
        out.append(struct.unpack_from("<" + "f" * ncomp, blob, base + i * 4 * ncomp))
    return out


def analyze(gltf_path):
    with open(gltf_path, "rb") as f:
        doc = json.loads(f.read().decode("utf-8"))
    bin_path = os.path.join(os.path.dirname(gltf_path), doc["buffers"][0]["uri"])
    with open(bin_path, "rb") as f:
        blob = f.read()

    roots = [i for i, n in enumerate(doc["nodes"]) if n.get("name") == "root"]
    if len(roots) != 1:
        raise SystemExit("%s: expected one node named 'root'" % gltf_path)
    root_i = roots[0]

    chan_t = None
    chan_r = None
    for c in doc["animations"][0]["channels"]:
        tgt = c.get("target") or {}
        if tgt.get("node") == root_i:
            if tgt.get("path") == "translation":
                chan_t = c
            elif tgt.get("path") == "rotation":
                chan_r = c
    if chan_t is None or chan_r is None:
        raise SystemExit("%s: missing root translation/rotation channels" % gltf_path)

    samp_t = doc["animations"][0]["samplers"][chan_t["sampler"]]
    times = [v[0] for v in read_accessor(doc, blob, samp_t["input"])]
    pos = read_accessor(doc, blob, samp_t["output"])
    rot = read_accessor(doc, blob, doc["animations"][0]["samplers"][chan_r["sampler"]]["output"])

    n = len(pos)
    duration = times[-1]
    dts = [times[i + 1] - times[i] for i in range(n - 1)]
    dt_uniform = max(abs(d - dts[0]) for d in dts) < 1e-5 if n > 1 else True

    first, last = pos[0], pos[-1]
    disp = tuple(last[k] - first[k] for k in range(3))
    horiz = math.hypot(disp[0], disp[2])
    vert = disp[1]
    ys = [p[1] for p in pos]
    descents_y = sum(1 for i in range(n - 1) if ys[i + 1] < ys[i] - 1e-6)

    hs = [math.hypot(p[0], p[2]) for p in pos]
    regress_h = sum(1 for i in range(n - 1) if hs[i + 1] < hs[i] - 1e-6)

    rev = 0
    for i in range(2, n):
        ax, az = pos[i - 1][0] - pos[i - 2][0], pos[i - 1][2] - pos[i - 2][2]
        bx, bz = pos[i][0] - pos[i - 1][0], pos[i][2] - pos[i - 1][2]
        if ax * bx + az * bz < 0:
            rev += 1

    rot_ident = all(all(abs(c) < 1e-6 for c in q[:3]) and abs(q[3] - 1.0) < 1e-6 for q in rot)

    notes = []
    notes.append("y_descents=%d/%d" % (descents_y, n - 1))
    notes.append("h_regressions=%d/%d" % (regress_h, n - 1))
    notes.append("h_reversals=%d" % rev)
    notes.append("uniform_dt=%s(dt=%.5f)" % (dt_uniform, dts[0] if n > 1 else 0.0))
    notes.append("rot_identity=%s" % rot_ident)

    return {
        "keys": n,
        "duration": duration,
        "disp": disp,
        "horiz": horiz,
        "vert": vert,
        "min_y": min(ys),
        "max_y": max(ys),
        "notes": ", ".join(notes),
    }


def main():
    ap = argparse.ArgumentParser(
        description="Validate root motion tracks written back into glTF animations.")
    ap.add_argument("--repo-root", required=True,
                    help="Path to the bevy_als repository root")
    args = ap.parse_args()

    rows = []
    failures = []
    for name, stem, horiz_rng, vert_rng, expect_dur in CLIPS:
        path = os.path.join(args.repo_root, "assets", "animations", stem + ".gltf")
        try:
            m = analyze(path)
        except (OSError, ValueError, KeyError, IndexError) as exc:
            raise SystemExit("FAIL %s: %s" % (name, exc))
        rows.append((name, m))

        checks = []
        checks.append(("duration", "%.4f" % m["duration"], abs(m["duration"] - expect_dur) < 1e-3))
        if horiz_rng is not None:
            ok = horiz_rng[0] <= m["horiz"] <= horiz_rng[1]
            checks.append(("horiz_m", "%.4f in [%s, %s]" % (m["horiz"], horiz_rng[0], horiz_rng[1]), ok))
        if vert_rng is not None:
            ok = vert_rng[0] <= m["vert"] <= vert_rng[1]
            checks.append(("vert_m", "%.4f in [%s, %s]" % (m["vert"], vert_rng[0], vert_rng[1]), ok))
        bad = [(k, v) for k, v, ok in checks if not ok]
        status = "PASS" if not bad else "FAIL"
        if bad:
            failures.append((name, bad))

        print("%s" % name)
        print("  keys=%d duration=%.4fs net_disp=(%+.4f, %+.4f, %+.4f)m horiz=%.4fm vert=%.4fm"
              % (m["keys"], m["duration"], m["disp"][0], m["disp"][1], m["disp"][2],
                 m["horiz"], m["vert"]))
        print("  y range [%.4f .. %.4f]  %s" % (m["min_y"], m["max_y"], m["notes"]))
        for k, v, ok in checks:
            print("  [%s] %-9s %s" % ("PASS" if ok else "FAIL", k, v))

    print("\nPASS criteria:")
    print("  A_Als_Roll horizontal net in [2.0, 4.0] m")
    print("  A_Als_Mantle_High vertical rise in [1.8, 2.2] m")
    print("  A_Als_Mantle_Low_* vertical rise in [0.3, 1.1] m")
    print("  durations unchanged: 2.667 s / 1.5 s")

    print("\nSUMMARY")
    for name, m in rows:
        print("  %-24s duration=%.4f horiz=%.4f vert=%.4f" %
              (name, m["duration"], m["horiz"], m["vert"]))

    if failures:
        print("\nVERDICT: FAIL (%d clip(s) outside stated bands)" % len(failures))
        for name, bad in failures:
            for k, v in bad:
                print("  %s: %s %s" % (name, k, v))
        sys.exit(1)
    print("\nVERDICT: PASS (all clips within stated bands)")


if __name__ == "__main__":
    main()
