import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from uasset_base import UAsset, ParseError


def import_class_name(a, ci):
    if ci < 0:
        imp = a.imports[-ci - 1]
        return imp.object_name if imp.class_name == "Class" else imp.class_name
    return None


def outer_tail(a, exp, depth=3):
    parts = []
    cur = exp.outer_index
    while cur > 0 and len(parts) < depth:
        pe = a.exports[cur - 1]
        parts.append(pe.object_name)
        cur = pe.outer_index
    return "/".join(reversed(parts))


def clean(v):
    if isinstance(v, dict):
        out = {}
        for k, vv in v.items():
            if k in ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"):
                fn = None
                if isinstance(vv, dict):
                    t = vv.get("__tagged__") or {}
                    fn = t.get("FunctionName")
                if fn and fn != "None":
                    out[k] = str(fn).split("/")[-1]
                continue
            out[k] = clean(vv)
        return out
    if isinstance(v, list):
        return [clean(x) for x in v]
    return v


def compact(props):
    keys = [
        "BoundGraph", "CustomTransitionGraph", "CustomBlendGraph", "Bidirectional",
        "TransitionType", "BlendMode", "CrossfadeDuration", "AutomaticRuleTriggerTime",
        "TransitionReturnThreshold", "SharedRules", "SharedFunctionName", "PriorityOrder",
        "PathRule", "ConduitType", "PinnedFromState", "bCanEnterTransition",
        "Node", "Tag", "Binding", "NodeName", "CacheName", "CachePoseName",
        "InputPoseNames", "InterfaceClass", "InstanceClass", "Layer", "LayerFunction",
        "SlotName", "RefPoseType", "ControlRigClass", "CurveName", "CurveNames",
        "ApplyMode", "AlphaInputType", "PlayRateScaleBiasClampConstants",
    ]
    out = {}
    for k in keys:
        if k in props:
            v = props[k]
            s = json.dumps(clean(v), default=str)
            out[k] = s[:400]
    if "Node" not in out:
        n = props.get("Node")
        if isinstance(n, dict) and "__tagged__" in n:
            t = n["__tagged__"]
            slim = {kk: clean(t[kk]) for kk in t if kk in (
                "Sequence", "PlayRate", "Position", "bLoop", "SlotName", "CacheName",
                "CachePoseName", "CurveName", "ApplyMode", "InstanceClass", "LayerFunction")}
            if slim:
                out["Node"] = json.dumps(slim, default=str)[:400]
    return out


def dump(path):
    a = UAsset(path)
    pr_reader = __import__("uasset_reader", fromlist=["PropReader"]).PropReader(a)
    print("=" * 110)
    print("ASSET %s" % path)
    funcs_by_graph = {}
    players = []
    caches = []
    states = []
    transitions = []
    misc = []
    for i, e in enumerate(a.exports):
        cls = import_class_name(a, e.class_index)
        tail = outer_tail(a, e)
        if cls == "K2Node_CallFunction":
            props, ok, err = pr_reader.read_export(i)
            if ok:
                fr = props.get("FunctionReference")
                name = None
                parent = None
                if isinstance(fr, dict):
                    t = fr.get("__tagged__", {})
                    name = str(t.get("MemberName"))
                    parent = str(t.get("MemberParent", "")).split("/")[-1]
                if name and name != "None":
                    funcs_by_graph.setdefault(tail, []).append("%s::%s" % (parent, name))
        elif cls in ("AnimGraphNode_SequencePlayer", "AnimGraphNode_SequenceEvaluator",
                     "AnimGraphNode_SequencePlayerBase"):
            props, ok, err = pr_reader.read_export(i)
            players.append((tail, cls, compact(props)))
        elif cls in ("AnimGraphNode_SaveCachedPose", "AnimGraphNode_UseCachedPose"):
            props, ok, err = pr_reader.read_export(i)
            caches.append((tail, cls, compact(props)))
        elif cls in ("AnimStateNode", "AnimStateAliasNode", "AnimStateConduitNode",
                     "AnimStateEntryNode"):
            props, ok, err = pr_reader.read_export(i)
            states.append((e.object_name, tail, cls, compact(props)))
        elif cls == "AnimStateTransitionNode":
            props, ok, err = pr_reader.read_export(i)
            transitions.append((e.object_name, tail, compact(props)))
        elif cls in ("AnimGraphNode_LinkedInputPose", "AnimGraphNode_ModifyCurve",
                     "AnimGraphNode_BlendSpacePlayer", "AnimGraphNode_ApplyMeshAdditive",
                     "AnimGraphNode_TwoBoneIK", "AnimGraphNode_CustomBlendGraph"):
            props, ok, err = pr_reader.read_export(i)
            misc.append((tail, cls, compact(props)))
    if states:
        print("-- states/aliases/conduits")
        for name, tail, cls, c in states:
            print("   %s [%s] %s %s" % (name, cls, tail.split("/")[-1] if tail else "", json.dumps(c)[:200]))
    if transitions:
        print("-- transitions")
        for name, tail, c in transitions:
            print("   %s %s" % (name, json.dumps(c)[:300]))
    if funcs_by_graph:
        print("-- rule/event functions by containing graph")
        for g in sorted(funcs_by_graph):
            short_g = "/".join(g.split("/")[-2:])
            uniq = sorted(set(funcs_by_graph[g]))
            print("   %s : %s" % (short_g, ", ".join(uniq)))
    if players:
        print("-- sequence players/evaluators")
        for tail, cls, c in players:
            print("   %s [%s] %s" % ("/".join(tail.split("/")[-2:]), cls, json.dumps(c)[:260]))
    if caches:
        print("-- cached poses")
        for tail, cls, c in caches:
            print("   %s [%s] %s" % ("/".join(tail.split("/")[-2:]), cls, json.dumps(c)[:200]))
    if misc:
        print("-- misc anim nodes")
        for tail, cls, c in misc:
            print("   %s [%s] %s" % ("/".join(tail.split("/")[-2:]), cls, json.dumps(c)[:240]))
    print()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="+")
    args = ap.parse_args()
    files = []
    for p in args.paths:
        if os.path.isdir(p):
            for root, _dirs, names in os.walk(p):
                for n in sorted(names):
                    if n.endswith(".uasset"):
                        files.append(os.path.join(root, n))
        else:
            files.append(p)
    for f in sorted(files):
        try:
            dump(f)
        except ParseError as exc:
            print("=" * 110)
            print("ASSET %s PARSE FAILED: %s" % (f, exc))
            print()


if __name__ == "__main__":
    main()
