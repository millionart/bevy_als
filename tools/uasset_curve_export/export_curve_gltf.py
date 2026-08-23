import argparse
import json
import os
import sys

from uasset import ParseError, UAsset
from extract_curves import RCIM_CUBIC, extract_float_curves, is_anim_sequence

EXTENSION_NAME = "ALS_float_curves"

INTERP_NAMES = {0: "LINEAR", 1: "CONSTANT", 2: "CUBIC", 3: "NONE"}
FLT_MAX_MARKER = 3.4028234663852886e+38


def find_content_roots(project_root):
    roots = [os.path.join(project_root, "Content")]
    plugins_dir = os.path.join(project_root, "Plugins")
    if os.path.isdir(plugins_dir):
        for entry in sorted(os.listdir(plugins_dir)):
            content = os.path.join(plugins_dir, entry, "Content")
            if os.path.isdir(content):
                roots.append(content)
    return [r for r in roots if os.path.isdir(r)]


def scan_uassets(content_roots):
    out = []
    for root in content_roots:
        for dirpath, _dirnames, filenames in os.walk(root):
            for fn in filenames:
                if fn.endswith(".uasset"):
                    out.append(os.path.join(dirpath, fn))
    return sorted(out)


def build_gltf_index(repo_root):
    anim_root = os.path.join(repo_root, "assets", "animations")
    index = {}
    if not os.path.isdir(anim_root):
        return index
    for dirpath, _dirnames, filenames in os.walk(anim_root):
        for fn in filenames:
            if fn.endswith(".gltf"):
                stem = fn[:-len(".gltf")]
                index.setdefault(stem, []).append(os.path.join(dirpath, fn))
    return index


def curve_to_json(curve):
    keys = curve.keys
    times = [k.time for k in keys]
    values = [k.value for k in keys]
    interps = [INTERP_NAMES.get(k.interp, str(k.interp)) for k in keys]
    default = None if curve.default_value >= FLT_MAX_MARKER else curve.default_value
    entry = {
        "name": curve.name,
        "flags": curve.flags,
        "default": default,
        "pre_infinity": curve.pre_infinity,
        "post_infinity": curve.post_infinity,
        "interps": interps,
        "times": times,
        "values": values,
    }
    needs_tangents = any(k.interp == RCIM_CUBIC or k.weight_mode != 0 or
                         k.arrive_tangent != 0.0 or k.leave_tangent != 0.0 for k in keys)
    if needs_tangents:
        entry["tangent_modes"] = [k.tangent_mode for k in keys]
        entry["weight_modes"] = [k.weight_mode for k in keys]
        entry["arrive_tangents"] = [k.arrive_tangent for k in keys]
        entry["leave_tangents"] = [k.leave_tangent for k in keys]
        entry["arrive_weights"] = [k.arrive_weight for k in keys]
        entry["leave_weights"] = [k.leave_weight for k in keys]
    return entry


def merge_into_gltf(gltf_path, curves):
    with open(gltf_path, "r", encoding="utf-8") as f:
        doc = json.load(f)
    animations = doc.get("animations") or []
    if len(animations) != 1:
        raise ParseError("%s: expected exactly one animation, found %d" % (gltf_path, len(animations)))
    used = doc.setdefault("extensionsUsed", [])
    if EXTENSION_NAME not in used:
        used.append(EXTENSION_NAME)
    anim = animations[0]
    ext_map = anim.setdefault("extensions", {})
    ext_map[EXTENSION_NAME] = {"curves": [curve_to_json(c) for c in curves]}
    tmp_path = gltf_path + ".tmp"
    with open(tmp_path, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent="\t")
    os.replace(tmp_path, gltf_path)


def process_asset(path, gltf_index, apply):
    try:
        asset = UAsset(path)
    except ParseError as exc:
        return {"asset": path, "status": "skip_unsupported", "detail": str(exc)}
    results = []
    for idx in range(len(asset.exports)):
        if not is_anim_sequence(asset, idx):
            continue
        export_name = asset.exports[idx].object_name
        curves = extract_float_curves(asset, idx)
        matches = gltf_index.get(export_name, [])
        record = {
            "asset": path,
            "object": export_name,
            "curves": [{"name": c.name, "keys": len(c.keys)} for c in curves],
            "n_curves": len(curves),
        }
        if not matches:
            record["status"] = "no_gltf_match"
        elif len(matches) > 1:
            record["status"] = "ambiguous_gltf_match"
            record["matches"] = matches
        else:
            record["status"] = "matched"
            record["gltf"] = matches[0]
            if apply:
                merge_into_gltf(matches[0], curves)
        results.append(record)
    if not results:
        return {"asset": path, "status": "no_anim_sequence_export"}
    return results


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Extract embedded float curves from UE animation .uasset files "
                    "into ALS_float_curves glTF animation extensions.")
    parser.add_argument("--project-root", required=True,
                        help="Path to the Unreal project directory containing .uproject")
    parser.add_argument("--repo-root", default=os.getcwd(),
                        help="bevy_als repository root whose assets/animations tree is updated")
    parser.add_argument("--apply", action="store_true",
                        help="Write merged glTF files; default is a dry run")
    parser.add_argument("--filter", default=None,
                        help="Substring filter on asset file path")
    parser.add_argument("--log", default=None,
                        help="Optional JSON report output path")
    args = parser.parse_args(argv)

    project_root = os.path.abspath(args.project_root)
    repo_root = os.path.abspath(args.repo_root)
    content_roots = find_content_roots(project_root)
    if not content_roots:
        print("error: no Content directories under %s" % project_root, file=sys.stderr)
        return 2
    gltf_index = build_gltf_index(repo_root)

    stats = {"scanned": 0, "anim_assets": 0, "with_curves": 0,
             "total_curves": 0, "matched": 0, "no_gltf_match": 0}
    report = []
    for path in scan_uassets(content_roots):
        if args.filter and args.filter not in path:
            continue
        stats["scanned"] += 1
        records = process_asset(path, gltf_index, args.apply)
        if isinstance(records, dict):
            records = [records]
        for rec in records:
            status = rec.get("status")
            if status == "no_anim_sequence_export":
                continue
            if status in ("skip_unsupported",):
                continue
            stats["anim_assets"] += 1
            n = rec.get("n_curves", 0)
            if n:
                stats["with_curves"] += 1
                stats["total_curves"] += n
                names = [c["name"] for c in rec["curves"]]
                print("%s: %d curves: %s" % (
                    os.path.basename(rec.get("asset", "?")), n, ", ".join(names)))
            if status == "matched":
                stats["matched"] += 1
            elif status == "no_gltf_match" and n:
                stats["no_gltf_match"] += 1
                print("NO MATCH %s (%s)" % (rec.get("asset"), rec.get("object")))
        report.extend(records)

    mode = "apply" if args.apply else "dry-run"
    print("MODE=%s scanned=%d anim_assets=%d assets_with_curves=%d total_curves=%d matched=%d unmatched_with_curves=%d"
          % (mode, stats["scanned"], stats["anim_assets"], stats["with_curves"],
             stats["total_curves"], stats["matched"], stats["no_gltf_match"]))
    if args.log:
        os.makedirs(os.path.dirname(os.path.abspath(args.log)), exist_ok=True)
        with open(args.log, "w", encoding="utf-8") as f:
            json.dump({"mode": mode, "stats": stats, "records": report}, f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
