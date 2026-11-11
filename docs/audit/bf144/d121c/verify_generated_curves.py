from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "tools/animgraph_authoring"))
from regenerate_installed_corpus import inventory
from ue_animation_asset_resolver import native_float_curve, rich_curve_key, asset_property
from ue_animgraph_materializer import rich_curve_ron
from ue_exported_text import field, parse_list, parse_struct


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def verify(corpus):
    evidence = Path(__file__).resolve().parent
    curves = []
    for path in sorted(evidence.glob("*.native.json")):
        source = read(path)
        if source["asset_class"] != "/Script/Engine.CurveFloat":
            continue
        old_keys = parse_list(field(parse_struct(asset_property(source, "FloatCurve")), "Keys"))
        old = rich_curve_ron([rich_curve_key(key) for key in old_keys])
        new = rich_curve_ron(native_float_curve(source)[0])
        if old != new:
            curves.append((source["asset_path"], old, new))
    progress = read(corpus / "corpus-progress.json")
    assert progress["roots"] == len(progress["results"]) == 15
    files = {}
    sources = set()
    for index, status in progress["results"].items():
        assert status["success"]
        folder = corpus / f"{int(index):02d}"
        _, generated = inventory(folder / "install")
        for name, data in generated.items():
            assert files.setdefault(name, data) == data, name
        report = read(folder / "generated/conversion.report.json")
        assert report["pass"] and not report["unsupported"]
        sources.add(report["source"]["asset"])
        sources.update(row["source_asset"] for row in report.get("linked_sources", []))
    assert len(sources) == 23
    diff = read(corpus / "corpus-diff.json")
    assert diff["installed_files"] == diff["generated_files"] == 374
    changed = []
    uses = Counter()
    for row in diff["changes"]:
        path = row["file"]
        before = (ROOT / "assets" / path).read_bytes()
        after = files[path]
        assert hashlib.sha256(before).hexdigest() == row["before_sha256"], path
        assert hashlib.sha256(after).hexdigest() == row["after_sha256"], path
        expected = before.decode()
        actual = after.decode()
        applied = []
        for asset, old, new in curves:
            if old in expected and new in actual:
                count = expected.count(old)
                expected = expected.replace(old, new)
                uses[asset] += count
                applied.append(asset)
        assert applied and expected == actual, path
        changed.append({"file":path, "curves":applied})
    assert len(changed) == 17 and len(uses) == 5
    return {"roots":15, "source_assets":sorted(sources), "materialized_files":374,
            "changed_files":changed, "curve_uses":dict(sorted(uses.items()))}


if __name__ == "__main__":
    print(json.dumps(verify(Path(sys.argv[1])), indent=2))
