import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from uasset_base import UAsset, ParseError
from uasset_reader import PropReader


def obj_path(a, index):
    if index == 0:
        return None
    parts = []
    cur = index
    depth = 0
    while cur != 0 and depth < 24:
        if cur < 0:
            imp = a.imports[-cur - 1]
            parts.append(imp.object_name)
            cur = imp.outer_index
            if cur == 0:
                if imp.package_name:
                    parts.append(imp.package_name)
                break
        else:
            exp = a.exports[cur - 1]
            parts.append(exp.object_name)
            cur = exp.outer_index
        depth += 1
    return "/".join(reversed(parts))


def export_class(a, index):
    ci = a.exports[index].class_index
    if ci < 0:
        imp = a.imports[-ci - 1]
        if imp.class_name == "Class":
            return imp.object_name
        return imp.class_name
    if ci > 0:
        sub = a.exports[ci - 1]
        sci = sub.class_index
        if sci < 0:
            simp = a.imports[-sci - 1]
            return simp.object_name if simp.class_name == "Class" else simp.class_name
        return sub.object_name
    return None


def dump_asset(path, want_classes=None, want_exports=None):
    a = UAsset(path)
    print("=" * 100)
    print("ASSET %s (ue4=%s ue5=%s)" % (path, a.summary.get("ue4"), a.summary.get("ue5")))
    print("-- imports (%d)" % len(a.imports))
    seen = {}
    for imp in a.imports:
        key = (imp.class_name, imp.package_name)
        seen.setdefault(key, []).append(imp.object_name)
    for (cls, pkg), names in sorted(seen.items()):
        uniq = sorted(set(names))
        print("  [%s] from <%s>: %s" % (cls, pkg, ", ".join(uniq[:40])) + (" ... +%d" % (len(uniq) - 40) if len(uniq) > 40 else ""))
    print("-- exports (%d)" % len(a.exports))
    pr = PropReader(a)
    for i, e in enumerate(a.exports):
        cls = export_class(a, i)
        chain = []
        cur = e.outer_index
        depth = 0
        while cur > 0 and depth < 12:
            pe = a.exports[cur - 1]
            chain.append(pe.object_name)
            cur = pe.outer_index
            depth += 1
        outer = "/".join(reversed(chain))
        print("  [%d] %s :: %s   outer=%s" % (i + 1, e.object_name, cls, outer))
        match_c = want_classes and cls and any(w.lower() in cls.lower() for w in want_classes)
        match_e = want_exports and any(w.lower() == e.object_name.lower() for w in want_exports)
        if match_c or match_e:
            props, ok, err = pr.read_export(i)
            if ok:
                interesting = {
                    k: v for k, v in props.items()
                    if k not in ("EditorCategoryId", "NodePosX", "NodePosY", "NodeWidth", "NodeHeight",
                                 "NodeComment", "CommentColor", "bCommentBubbleVisible", "NodeGuid",
                                 "bCanRenameNode", "bCanResizeNode", "ErrorType", "ErrorMsg", "AdvancedPinDisplay",
                                 "EnabledState", "bIsNodeEnabled", "CallFunc", "DeprecatedGraph")
                }
                print("      props: %s" % (interesting,))
            else:
                print("      props: <parse failed: %s>" % err)
    print()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="+")
    ap.add_argument("--classes", default="")
    ap.add_argument("--exports", default="")
    args = ap.parse_args()
    wc = [s for s in args.classes.split(",") if s]
    we = [s for s in args.exports.split(",") if s]
    files = []
    for p in args.paths:
        if os.path.isdir(p):
            for root, _dirs, names in os.walk(p):
                for n in names:
                    if n.endswith(".uasset"):
                        files.append(os.path.join(root, n))
        else:
            files.append(p)
    for f in sorted(files):
        try:
            dump_asset(f, wc, we)
        except ParseError as exc:
            print("=" * 100)
            print("ASSET %s PARSE FAILED: %s" % (f, exc))
            print()


if __name__ == "__main__":
    main()
