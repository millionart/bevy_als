import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir, "uasset_curve_export"))

from uasset import Reader, UAsset


class TagError(Exception):
    pass


def fname_at(buf, pos, names):
    idx, num = struct.unpack_from("<ii", buf, pos)
    if idx < 0 or idx >= len(names):
        raise TagError("name index %d out of range at 0x%X" % (idx, pos))
    return names[idx] + ("_%d" % (num - 1) if num else "")


def read_node(buf, pos, names, depth=0):
    if depth > 8:
        raise TagError("type node nesting too deep")
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
            raise TagError("stream tag name index %d invalid at 0x%X" % (idx, pos))
        if names[idx] == "None":
            pos += 8
            break
        tag = read_tag(buf, pos, names)
        tag["tag_pos"] = pos
        tags.append(tag)
        pos = tag["payload_end"]
        if end_limit is not None and pos > end_limit:
            raise TagError("stream overran limit 0x%X at 0x%X" % (end_limit, pos))
    return tags, root_byte, pos


def find_tracks_tag_pos(asset, exp):
    want = struct.pack("<i", asset.names.index("BoneAnimationTracks"))
    base = exp.serial_offset
    limit = base + min(256, exp.serial_size)
    pos = base + 1
    while True:
        j = asset.buf.find(want, pos, limit)
        if j < 0:
            raise TagError("BoneAnimationTracks tag not found near export start")
        if struct.unpack_from("<i", asset.buf, j + 4)[0] == 0:
            return j
        pos = j + 1


def decode_track_element(asset, tags, payload_bounds):
    info = {"name": None, "bone_tree_index": None}
    track = None
    for t in tags:
        ps, pe = t["payload_start"], t["payload_end"]
        if t["name"] == "InternalTrackData":
            if not t["flags"] & 0x08:
                raise TagError("InternalTrackData lacks BinaryOrNativeSerialize flag")
            buf = asset.buf
            esz_p, n_p = struct.unpack_from("<ii", buf, ps)
            d_p = ps + 8
            d_p_end = d_p + n_p * esz_p
            esz_r, n_r = struct.unpack_from("<ii", buf, d_p_end)
            d_r = d_p_end + 8
            d_r_end = d_r + n_r * esz_r
            esz_s, n_s = struct.unpack_from("<ii", buf, d_r_end)
            d_s = d_r_end + 8
            d_s_end = d_s + n_s * esz_s
            if d_s_end != pe:
                raise TagError("track bulk arithmetic mismatch 0x%X != 0x%X" % (d_s_end, pe))
            track = {
                "pos": [struct.unpack_from("<3f", buf, d_p + 12 * i) for i in range(n_p)],
                "rot": [struct.unpack_from("<4f", buf, d_r + 16 * i) for i in range(n_r)],
                "scale_n": n_s,
                "pos_n": n_p,
                "rot_n": n_r,
            }
        elif t["name"] == "Name":
            info["name"] = fname_at(asset.buf, ps, asset.names)
        elif t["name"] == "BoneTreeIndex":
            info["bone_tree_index"] = struct.unpack_from("<i", asset.buf, ps)[0]
    if track is None:
        raise TagError("element without InternalTrackData")
    if info["name"] is None:
        raise TagError("element without Name")
    return track, info


def scalar_i32(asset, tag):
    if tag["payload_end"] - tag["payload_start"] != 4:
        raise TagError("%s payload not 4 bytes" % tag["name"])
    return struct.unpack_from("<i", asset.buf, tag["payload_start"])[0]


def main():
    ap = argparse.ArgumentParser(description="Probe AnimDataModel raw bone track serialization layout.")
    ap.add_argument("--uasset", required=True)
    args = ap.parse_args()

    asset = UAsset(args.uasset)
    buf = asset.buf
    names = asset.names
    print("package: %s" % args.uasset)
    print("versions ue4=%d ue5=%d names=%d imports=%d exports=%d" %
          (asset.summary["ue4"], asset.summary["ue5"], len(names),
           len(asset.imports), len(asset.exports)))

    adm = None
    seq = None
    for i, e in enumerate(asset.exports):
        ci = e.class_index
        cls = None
        if ci < 0:
            imp = asset.imports[-ci - 1]
            cls = imp.object_name if imp.class_name == "Class" else imp.class_name
        elif ci > 0:
            cls = asset.exports[ci - 1].object_name
        marker = ""
        if cls == "AnimDataModel":
            adm = i
            marker = "  <-- AnimDataModel"
        elif cls == "AnimSequence":
            seq = i
            marker = "  <-- AnimSequence"
        print("export %2d: name=%-30r class=%-24r serial_size=%7d offset=0x%X%s"
              % (i, e.object_name, cls, e.serial_size, e.serial_offset, marker))

    if adm is None:
        raise SystemExit("no AnimDataModel export found")

    exp = asset.exports[adm]
    end = exp.serial_offset + exp.serial_size

    tags, root_byte, after = walk_stream(buf, exp.serial_offset, names, end, root_byte=True)
    consumed = after - exp.serial_offset
    print("\nexport body: root extensions byte=0x%02X, walked 0x%X..0x%X (%d of %d bytes)%s"
          % (root_byte, exp.serial_offset, after, consumed, exp.serial_size,
             "" if consumed == exp.serial_size else "  <-- RESIDUAL BYTES"))

    tracks_tag = None
    meta = {}
    for t in tags:
        print("prop %-22s type=%-56s size=%7d flags=0x%02X range=0x%X..0x%X"
              % (t["name"], t["type"], t["size"], t["flags"], t["payload_start"], t["payload_end"]))
        if t["name"] == "BoneAnimationTracks":
            tracks_tag = t
        elif t["name"] == "NumberOfKeys":
            meta["NumberOfKeys"] = scalar_i32(asset, t)
        elif t["name"] == "NumberOfFrames":
            meta["NumberOfFrames"] = scalar_i32(asset, t)
        elif t["name"] == "FrameRate":
            fr = {}
            subtags, _, _ = walk_stream(buf, t["payload_start"], names, t["payload_end"])
            for st in subtags:
                fr[st["name"]] = scalar_i32(asset, st)
            meta["FrameRate"] = fr

    print("\nmeta: NumberOfKeys=%s NumberOfFrames=%s FrameRate=%s" %
          (meta.get("NumberOfKeys"), meta.get("NumberOfFrames"), meta.get("FrameRate")))

    if tracks_tag is None:
        raise SystemExit("BoneAnimationTracks property missing")

    ps, pe = tracks_tag["payload_start"], tracks_tag["payload_end"]
    count = struct.unpack_from("<i", buf, ps)[0]
    print("\nBoneAnimationTracks payload 0x%X..0x%X (%d bytes), track count=%d"
          % (ps, pe, pe - ps, count))

    first_elem_stream_start = ps + 4
    etags, eroot, eend = walk_stream(buf, first_elem_stream_start, names)
    etag = next(t for t in etags if t["name"] == "InternalTrackData")
    print("\nelement[0] stream: root byte=0x%02X, %d tags, ends 0x%X" % (eroot, len(etags), eend))
    for t in etags:
        print("  tag %-18r type=%-58s size=%6d flags=0x%02X range=0x%X..0x%X"
              % (t["name"], t["type"], t["size"], t["flags"], t["payload_start"], t["payload_end"]))

    print("\nfirst element InternalTrackData payload hexdump:")
    hs = etag["payload_start"]
    for row in range(0, 80, 16):
        chunk = buf[hs + row:hs + row + 16]
        hexs = " ".join("%02X" % b for b in chunk)
        text = "".join(chr(b) if 32 <= b < 127 else "." for b in chunk)
        print("  %08X  %-47s  %s" % (hs + row, hexs, text))

    head = struct.unpack_from("<ii", buf, hs)
    if etag["flags"] & 0x08 and head[0] == 12:
        verdict = ("BULK (int32 ElementSize; int32 ArrayNum; raw little-endian floats); "
                   "evidence: tag flags HasBinaryOrNativeSerialize=0x08 at 0x%X, "
                   "payload head ints (ElementSize=%d, ArrayNum=%d)"
                   % (etag["tag_pos"], head[0], head[1]))
    elif etag["flags"] & 0x08:
        verdict = "native-flag set but unexpected head ints %s" % (head,)
    else:
        verdict = "TAGGED ArrayProperty streams (flag 0x08 absent)"
    print("\nVERDICT: InternalTrackData arrays serialize as %s" % verdict)

    print("\n--- decoding all %d track elements ---" % count)
    pos = ps + 4
    root_info = None
    pelvis_info = None
    key_counts = set()
    for k in range(count):
        etags, _, eend = walk_stream(buf, pos, names, pe)
        track, info = decode_track_element(asset, etags, (pos, pe))
        key_counts.add((track["pos_n"], track["rot_n"], track["scale_n"]))
        if info["name"] == "root":
            root_info = (k, info, track)
        if info["name"] == "pelvis":
            pelvis_info = (k, info, track)
        pos = eend
    if pos != pe:
        print("WARNING: elements walked to 0x%X but property ends 0x%X" % (pos, pe))
    else:
        print("all %d elements decoded cleanly; payload fully consumed" % count)
    print("key-count tuples across all tracks: %s" % sorted(key_counts))

    if root_info:
        k, info, tr = root_info
        ident = all(abs(q[3] - 1.0) < 1e-6 and abs(q[0]) < 1e-6 and
                    abs(q[1]) < 1e-6 and abs(q[2]) < 1e-6 for q in tr["rot"])
        print("\nroot track: element %d bone_tree_index=%s pos_keys=%d rot_keys=%d rot_all_identity=%s"
              % (k, info["bone_tree_index"], tr["pos_n"], tr["rot_n"], ident))
        print("  pos[0] cm   = %s" % (tuple(round(v, 4) for v in tr["pos"][0]),))
        print("  pos[mid] cm = %s" % (tuple(round(v, 4) for v in tr["pos"][tr["pos_n"] // 2]),))
        print("  pos[-1] cm  = %s" % (tuple(round(v, 4) for v in tr["pos"][-1]),))
        print("  rot[0] xyzw = %s" % (tuple(round(v, 5) for v in tr["rot"][0]),))
    else:
        print("\nNO track named 'root' found!")

    if pelvis_info:
        k, info, tr = pelvis_info
        print("\npelvis track: element %d bone_tree_index=%s pos_keys=%d rot_keys=%d"
              % (k, info["bone_tree_index"], tr["pos_n"], tr["rot_n"]))
        step = max(1, tr["pos_n"] // 4)
        for i in range(0, tr["pos_n"], step):
            print("  pos[%d] cm = %s" % (i, tuple(round(v, 3) for v in tr["pos"][i])))
        print("  rot[0] xyzw = %s" % (tuple(round(v, 5) for v in tr["rot"][0]),))


if __name__ == "__main__":
    main()
