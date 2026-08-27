"""Extract curve names + default values from ALL ModifyCurve nodes across AB_Als_Camera graphs.

These are the camera anim-instance curves the ShowDebug CameraCurves panel lists.
Values here are per-state defaults; runtime values come from gameplay (lean/aim/overlay).
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROBE = ROOT / "tools" / "mcp_probe.py"
BP_ROOT = "/ALS/ALSCamera/AB_Als_Camera.AB_Als_Camera"


def call(toolset, tool, args):
    r = subprocess.run(
        [sys.executable, str(PROBE), "call", toolset, tool, json.dumps(args)],
        capture_output=True, text=True, timeout=300,
    )
    out = r.stdout.strip()
    try:
        return json.loads(out)
    except Exception:
        return {"raw": out[:1500]}


def node_curves(ref: str) -> dict | None:
    bp_tools = "editor_toolset.toolsets.blueprint.BlueprintTools"
    nodes = call(bp_tools, "find_nodes", {"graph": {"refPath": ref}, "title": ""})
    modify_refs = [
        n["refPath"] for n in nodes.get("returnValue", [])
        if "ModifyCurve" in n.get("refPath", "")
    ]
    if not modify_refs:
        return None
    obj_tools = "editor_toolset.toolsets.object.ObjectTools"
    curves: dict[str, float] = {}
    modes: dict[str, str] = {}
    for mref in modify_refs:
        r = call(obj_tools, "get_properties", {
            "instance": {"refPath": mref},
            "properties": ["node"],
        })
        try:
            node = json.loads(r["returnValue"])["node"]
            names = node.get("curveNames", [])
            values = node.get("curveValues", [])
            for n, v in zip(names, values):
                curves[n] = v
                modes[mref.split("_")[-1]] = node.get("applyMode", "?")
        except Exception:
            continue
    return {"curves": curves, "modes": modes} if curves else None


def main() -> None:
    bp_tools = "editor_toolset.toolsets.blueprint.BlueprintTools"
    graphs = call(bp_tools, "list_graphs", {"blueprint": {
        "refPath": "/ALS/ALSCamera/AB_Als_Camera.AB_Als_Camera"}})

    merged: dict[str, float] = {}
    detail: dict[str, dict] = {}
    for g in graphs.get("returnValue", []):
        ref = g["refPath"]
        name = ref.split(":")[-1]
        res = node_curves(ref)
        if res:
            detail[name] = res
            merged.update(res["curves"])

    with (ROOT / "docs" / "bf9-camera-curve-names.json").open("w", encoding="utf-8") as f:
        json.dump({"merged_curve_defaults": merged, "per_graph_detail": detail}, f, indent=1)
    print(json.dumps(sorted(merged.keys()), indent=1))
    print("total distinct curves:", len(merged))


if __name__ == "__main__":
    main()
