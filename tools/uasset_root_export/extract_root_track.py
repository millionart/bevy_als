import argparse
import json
import math
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir, "uasset_curve_export"))

from uasset import ParseError, Reader, UAsset


class ExtractError(Exception):
    pass


CLIPS = [
    ("A_Als_Mantle_High", "Actions/Mantle/A_Als_Mantle_High"),
    ("A_Als_Mantle_Low_Left", "Actions/Mantle/A_Als_Mantle_Low_Left"),
    ("A_Als_Mantle_Low_Right", "Actions/Mantle/A_Als_Mantle_Low_Right"),
    ("A_Als_Roll", "Actions/Roll/A_Als_Roll"),
]


def fname_at(buf, pos, names):
    idx, num = struct.unpack_from("<ii", buf, pos)
    if idx < 0 or idx >= len(names):
        raise ExtractError("name index %d out of range at 0x%X" % (idx, pos))
    return names[idx] + ("_%d" % (num - 1) if num else "")


def read_node(buf, pos, names, depth=0):
    if depth > 8:
        raise ExtractError("type node nesting too deep")
    idx, num, count = struct.unpack_from("<iii", buf, pos)
    pos += 12
    children = []
    for _ in range(count):
        child, pos = read_node(buf, pos, names, depth + 1)
        children.append(child)
    return (idx, num, count, children), pos


def type_text(names, node):
    idx, num, _, children = node
    text = names[idx] + ("_%d" % (num - 1) if num else "")
    for ch in children:
        text += "." + type_text(names, ch)
    return text


def read_tag(buf, pos, names):
    name = fname_at(buf, pos, names)
    pos += 8
    root, pos = read_node(buf, pos, names)
    size = struct.unpack_from("<i", buf, pos)[0]
    pos += 4
    flags = buf[pos]
    pos += 1
    if flags & 0x01:
        pos += 4
    if flags & 0x02:
        pos += 16
    if flags & 0x04:
        ext_bits = buf[pos]
        pos += 1
        if ext_bits & 0x02:
            pos += 5
        if ext_bits & 0x04:
            pos += 4
    return {
        "name": name,
        "type": type_text(names, root),
        "size": size,
        "flags": flags,
        "payload_start": pos,
        "payload_end": pos + size,
    }


def walk_stream(buf, pos, names, end_limit=None, root_byte=False):
    rb = None
    if root_byte:
        rb = buf[pos]
        pos += 1
    tags = []
    while True:
        idx = struct.unpack_from("<i", buf, pos)[0]
        if idx < 0 or idx >= len(names):
            raise ExtractError("stream tag name index %d invalid at 0x%X" % (idx, pos))
        if names[idx] == "None":
            pos += 8
            break
        tag = read_tag(buf, pos, names)
        tag["tag_pos"] = pos
        tags.append(tag)
        pos = tag["payload_end"]
        if end_limit is not None and pos > end_limit:
            raise ExtractError("stream overran limit 0x%X at 0x%X" % (end_limit, pos))
    return tags, rb, pos


def find_anim_data_model(asset):
    for i, e in enumerate(asset.exports):
        ci = e.class_index
        if ci < 0:
            imp = asset.imports[-ci - 1]
            cls = imp.object_name if imp.class_name == "Class" else imp.class_name
            if cls == "AnimDataModel":
                return i
    raise ExtractError("%s: no AnimDataModel export" % asset.path)


def decode_anim_data_model(asset):
    exp = asset.exports[find_anim_data_model(asset)]
    end = exp.serial_offset + exp.serial_size
    tags, _, _ = walk_stream(asset.buf, exp.serial_offset, asset.names, end, root_byte=True)
    tracks_tag = None
    meta = {}
    for t in tags:
        if t["name"] == "BoneAnimationTracks":
            tracks_tag = t
        elif t["name"] == "NumberOfKeys":
            meta["number_of_keys"] = struct.unpack_from("<i", asset.buf, t["payload_start"])[0]
        elif t["name"] == "FrameRate":
            subtags, _, _ = walk_stream(asset.buf, t["payload_start"], asset.names, t["payload_end"])
            fr = {}
            for st in subtags:
                if st["payload_end"] - st["payload_start"] == 4:
                    fr[st["name"]] = struct.unpack_from("<i", asset.buf, st["payload_start"])[0]
            meta["frame_rate"] = fr
    if tracks_tag is None:
        raise ExtractError("%s: BoneAnimationTracks missing" % asset.path)
    ps, pe = tracks_tag["payload_start"], tracks_tag["payload_end"]
    count = struct.unpack_from("<i", asset.buf, ps)[0]
    tracks = {}
    order = []
    pos = ps + 4
    for _ in range(count):
        etags, _, eend = walk_stream(asset.buf, pos, asset.names, pe)
        tr = None
        info = {"name": None, "bone_tree_index": None}
        for t in etags:
            pps, ppe = t["payload_start"], t["payload_end"]
            if t["name"] == "InternalTrackData":
                if not t["flags"] & 0x08:
                    raise ExtractError("InternalTrackData lacks native serialize flag")
                buf = asset.buf
                esz_p, n_p = struct.unpack_from("<ii", buf, pps)
                d_p = pps + 8
                d_pe = d_p + n_p * esz_p
                esz_r, n_r = struct.unpack_from("<ii", buf, d_pe)
                d_r = d_pe + 8
                d_re = d_r + n_r * esz_r
                esz_s, n_s = struct.unpack_from("<ii", buf, d_re)
                d_s = d_re + 8
                d_se = d_s + n_s * esz_s
                if esz_p != 12 or esz_r != 16 or esz_s != 12 or d_se != ppe:
                    raise ExtractError("bulk array arithmetic mismatch in element at 0x%X" % t["tag_pos"])
                tr = {
                    "pos": [struct.unpack_from("<3f", buf, d_p + 12 * i) for i in range(n_p)],
                    "rot": [struct.unpack_from("<4f", buf, d_r + 16 * i) for i in range(n_r)],
                    "scale_n": n_s,
                }
            elif t["name"] == "Name":
                info["name"] = fname_at(asset.buf, pps, asset.names)
            elif t["name"] == "BoneTreeIndex":
                info["bone_tree_index"] = struct.unpack_from("<i", asset.buf, pps)[0]
        if tr is None or info["name"] is None:
            raise ExtractError("incomplete bone track element")
        tracks[info["name"]] = tr
        order.append(info)
        pos = eend
    if pos != pe:
        raise ExtractError("%s: tracks payload walked to 0x%X but ends 0x%X" % (asset.path, pos, pe))
    meta["track_count"] = count
    meta["tracks"] = tracks
    meta["order"] = order
    return meta


def convert_translation(key_cm):
    x, y, z = key_cm
    return (x * 0.01, z * 0.01, y * 0.01)


SWAP = (
    (1.0, 0.0, 0.0),
    (0.0, 0.0, 1.0),
    (0.0, 1.0, 0.0),
)


def quat_to_mat(q):
    x, y, z, w = q
    return (
        (1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)),
        (2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)),
        (2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)),
    )


def mat_mul(a, b):
    return tuple(tuple(sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)) for i in range(3))


def mat_transpose(m):
    return tuple(tuple(m[j][i] for j in range(3)) for i in range(3))


def mat_to_quat(m):
    tr_ = m[0][0] + m[1][1] + m[2][2]
    if tr_ > 0:
        s = math.sqrt(tr_ + 1.0) * 2
        return ((m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s, 0.25 * s)
    if m[0][0] > m[1][1] and m[0][0] > m[2][2]:
        s = math.sqrt(1.0 + m[0][0] - m[1][1] - m[2][2]) * 2
        return (0.25 * s, (m[0][1] + m[1][0]) / s, (m[0][2] + m[2][0]) / s, (m[2][1] - m[1][2]) / s)
    if m[1][1] > m[2][2]:
        s = math.sqrt(1.0 + m[1][1] - m[0][0] - m[2][2]) * 2
        return ((m[0][1] + m[1][0]) / s, 0.25 * s, (m[1][2] + m[2][1]) / s, (m[0][2] - m[2][0]) / s)
    s = math.sqrt(1.0 + m[2][2] - m[0][0] - m[1][1]) * 2
    return ((m[0][2] + m[2][0]) / s, (m[1][2] + m[2][1]) / s, 0.25 * s, (m[1][0] - m[0][1]) / s)


def convert_rotation(q):
    r_g = mat_mul(mat_mul(SWAP, quat_to_mat(q)), mat_transpose(SWAP))
    out = mat_to_quat(r_g)
    if out[3] < 0:
        out = tuple(-v for v in out)
    return out


def load_gltf(gltf_path):
    with open(gltf_path, "rb") as f:
        raw = f.read()
    doc = json.loads(raw.decode("utf-8"))
    bin_uri = doc["buffers"][0]["uri"]
    bin_path = os.path.join(os.path.dirname(gltf_path), bin_uri)
    with open(bin_path, "rb") as f:
        blob = f.read()
    return doc, blob, bin_path


def accessor_slice(doc, blob, acc_idx):
    acc = doc["accessors"][acc_idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    ncomp = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[acc["type"]]
    base = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    out = []
    for i in range(acc["count"]):
        out.append(struct.unpack_from("<" + "f" * ncomp, blob, base + i * 4 * ncomp))
    return out


def find_root_channels(doc):
    nodes = doc["nodes"]
    roots = [i for i, n in enumerate(nodes) if n.get("name") == "root"]
    if len(roots) != 1:
        raise ExtractError("expected exactly one node named 'root', found %d" % len(roots))
    root_i = roots[0]
    anims = doc.get("animations") or []
    if len(anims) != 1:
        raise ExtractError("expected exactly one animation, found %d" % len(anims))
    chan_t = None
    chan_r = None
    for c in anims[0]["channels"]:
        tgt = c.get("target") or {}
        if tgt.get("node") == root_i and tgt.get("path") in ("translation", "rotation"):
            if tgt["path"] == "translation":
                chan_t = c
            else:
                chan_r = c
    if chan_t is None or chan_r is None:
        raise ExtractError("root node lacks translation/rotation channels")
    return root_i, chan_t, chan_r


def process_clip(name, stem, als_content, repo_root):
    uasset_path = os.path.join(als_content, "ALS", "Animations", stem + ".uasset")
    gltf_path = os.path.join(repo_root, "assets", "animations", stem + ".gltf")
    asset = UAsset(uasset_path)
    model = decode_anim_data_model(asset)

    if "root" not in model["tracks"]:
        raise ExtractError("%s: no track named 'root'" % name)
    root_track = model["tracks"]["root"]
    n = len(root_track["pos"])
    if len(root_track["rot"]) != n:
        raise ExtractError("%s: pos/rot key count mismatch" % name)
    fr = model.get("frame_rate") or {}
    numerator = fr.get("Numerator")
    denominator = fr.get("Denominator", 1)
    if not numerator or numerator <= 0 or denominator <= 0:
        raise ExtractError("%s: invalid FrameRate %s" % (name, fr))
    fps = float(numerator) / float(denominator)
    if model.get("number_of_keys") not in (None, n):
        raise ExtractError("%s: NumberOfKeys=%s but root track has %d keys"
                           % (name, model.get("number_of_keys"), n))

    times = [i / fps for i in range(n)]
    translations = [convert_translation(k) for k in root_track["pos"]]
    rotations = [convert_rotation(k) for k in root_track["rot"]]

    doc, blob, bin_path = load_gltf(gltf_path)
    root_i, chan_t, chan_r = find_root_channels(doc)
    anim = doc["animations"][0]
    samp_t = anim["samplers"][chan_t["sampler"]]
    samp_r = anim["samplers"][chan_r["sampler"]]
    if samp_t["input"] != samp_r["input"]:
        raise ExtractError("%s: root samplers use different time accessors" % name)
    existing_times = accessor_slice(doc, blob, samp_t["input"])
    if len(existing_times) != n:
        raise ExtractError("%s: gltf time grid has %d keys, uasset has %d"
                           % (name, len(existing_times), n))
    for i in range(n):
        if abs(existing_times[i][0] - times[i]) > 1e-4:
            raise ExtractError("%s: time grid mismatch at %d: gltf %.6f vs derived %.6f"
                               % (name, i, existing_times[i][0], times[i]))

    flat_t = b"".join(struct.pack("<3f", *v) for v in translations)
    flat_r = b"".join(struct.pack("<4f", *v) for v in rotations)
    old_len = len(blob)
    pad = (-old_len) % 4
    off_t = old_len + pad
    off_r = off_t + len(flat_t)
    new_blob_len = off_r + len(flat_r)

    mins_t = [min(v[k] for v in translations) for k in range(3)]
    maxs_t = [max(v[k] for v in translations) for k in range(3)]
    mins_r = [min(v[k] for v in rotations) for k in range(4)]
    maxs_r = [max(v[k] for v in rotations) for k in range(4)]

    new_doc = json.loads(json.dumps(doc))
    bv_base = len(new_doc["bufferViews"])
    acc_base = len(new_doc["accessors"])
    new_doc["bufferViews"].append({"buffer": 0, "byteOffset": off_t, "byteLength": len(flat_t)})
    new_doc["bufferViews"].append({"buffer": 0, "byteOffset": off_r, "byteLength": len(flat_r)})
    new_doc["accessors"].append({
        "bufferView": bv_base, "componentType": 5126, "count": n, "type": "VEC3",
        "min": mins_t, "max": maxs_t,
    })
    new_doc["accessors"].append({
        "bufferView": bv_base + 1, "componentType": 5126, "count": n, "type": "VEC4",
        "min": mins_r, "max": maxs_r,
    })
    new_doc["animations"][0]["samplers"][chan_t["sampler"]]["output"] = acc_base
    new_doc["animations"][0]["samplers"][chan_r["sampler"]]["output"] = acc_base + 1
    new_doc["buffers"][0]["byteLength"] = new_blob_len

    first = translations[0]
    last = translations[-1]
    disp = tuple(last[k] - first[k] for k in range(3))
    horiz = math.hypot(disp[0], disp[2])
    vert = disp[1]

    result = {
        "name": name,
        "fps": fps,
        "keys": n,
        "duration": (n - 1) / fps,
        "ue_start_cm": root_track["pos"][0],
        "ue_end_cm": root_track["pos"][-1],
        "net_disp": disp,
        "horiz": horiz,
        "vert": vert,
        "rot_identity": all(all(abs(c) < 1e-6 for c in q[:3]) and abs(q[3] - 1.0) < 1e-6
                            for q in root_track["rot"]),
        "old_bin": old_len,
        "new_bin": new_blob_len,
        "gltf_path": gltf_path,
        "bin_path": bin_path,
        "doc": new_doc,
        "blob_tail": b"\x00" * pad + flat_t + flat_r,
    }
    return result


def dump_gltf(doc):
    return json.dumps(doc, indent="\t", ensure_ascii=False).replace("\n", "\r\n").encode("utf-8")


def main():
    ap = argparse.ArgumentParser(
        description="Extract UE root-bone raw tracks from editor uassets and backfill glTF root channels.")
    ap.add_argument("--als-content", required=True,
                    help="Path to ALS-Refactored plugin Content directory")
    ap.add_argument("--repo-root", required=True,
                    help="Path to the bevy_als repository root")
    ap.add_argument("--apply", action="store_true",
                    help="Rewrite the gltf/bin pairs (default: dry run)")
    args = ap.parse_args()

    results = []
    for name, stem in CLIPS:
        try:
            results.append(process_clip(name, stem, args.als_content, args.repo_root))
        except (ParseError, ExtractError) as exc:
            raise SystemExit("FAIL %s: %s" % (name, exc))

    print("%-24s %6s %5s %9s  %-31s %7s %8s" %
          ("clip", "fps", "keys", "dur(s)", "net_disp(gltf xyz, m)", "horiz_m", "vert_m"))
    for r in results:
        print("%-24s %6.2f %5d %9.4f  (%+.4f, %+.4f, %+.4f)          %7.4f %8.4f" %
              (r["name"], r["fps"], r["keys"], r["duration"],
               r["net_disp"][0], r["net_disp"][1], r["net_disp"][2],
               r["horiz"], r["vert"]))
        print("%-24s ue_start_cm=%s ue_end_cm=%s rot_identity=%s bin %d -> %d bytes" %
              ("", r["ue_start_cm"], r["ue_end_cm"], r["rot_identity"],
               r["old_bin"], r["new_bin"]))

    if args.apply:
        for r in results:
            new_bin = None
            with open(r["bin_path"], "rb") as f:
                new_bin = f.read() + r["blob_tail"]
            with open(r["bin_path"], "wb") as f:
                f.write(new_bin)
            with open(r["gltf_path"], "wb") as f:
                f.write(dump_gltf(r["doc"]))
        print("\nAPPLIED: %d gltf/bin pairs rewritten" % len(results))
    else:
        print("\nDRY RUN: no files written")


if __name__ == "__main__":
    main()
