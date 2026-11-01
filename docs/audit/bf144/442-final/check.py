import copy
import hashlib
import json
import subprocess
from pathlib import Path

r = Path(__file__).resolve().parent
names = ("442", "442b0", "442b", "442c0", "442c1", "442c2")


def validate_children(children):
    assert set(children) == set(names)
    for child in children.values():
        assert child["gate"].startswith("PASS_")


def verify(raw, record):
    assert len(raw) == record["bytes"]
    assert hashlib.sha256(raw).hexdigest() == record["sha256"]


children = {}
total = 0
for name in names:
    root = r.parent / name
    summary = json.loads((root / "summary.json").read_text())
    manifest = json.loads((root / "evidence-manifest.json").read_text())
    for relative, record in manifest["files"].items():
        p = root / relative
        verify(p.read_bytes(), record)
        subprocess.run(["git", "ls-files", "--error-unmatch", str(p.relative_to(Path.cwd()))], check=True, stdout=subprocess.DEVNULL, timeout=5)
        total += 1
    children[name] = dict(gate=summary["gate"], files=len(manifest["files"]), summary_sha256=hashlib.sha256((root / "summary.json").read_bytes()).hexdigest(), manifest_sha256=hashlib.sha256((root / "evidence-manifest.json").read_bytes()).hexdigest())
    print("Verified sealed child " + name, flush=True)
validate_children(children)
for mutation in ("missing", "failed"):
    bad = copy.deepcopy(children)
    if mutation == "missing":
        del bad["442c2"]
    else:
        bad["442c2"]["gate"] = "FAIL"
    try:
        validate_children(bad)
    except AssertionError:
        continue
    raise AssertionError(mutation)
try:
    verify(b"changed", dict(bytes=7, sha256="0" * 64))
except AssertionError:
    pass
else:
    raise AssertionError("Changed evidence was accepted")
result = dict(gate="PASS_SCOPED_INPUT_DIRECTION_AUDIT_WITH_RECORDED_DIFFERENCES", children=children, verified_git_evidence_files=total, negative_child_status_and_hash_cases_rejected=3, new_differences=["BF144-D269", "BF144-D270", "BF144-D271", "BF144-D272"], differences_fixed=False, total_confirmed_differences=269, priority_counts=dict(P2=116, P3=153), full_als_equivalence_claimed=False, native_source_warm_lifecycle_claimed=False, natural_nonzero_cold_input_claimed=False, full_world_feedback_claimed=False, screenshots=0, performance_acceptance="paused")
(r / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({k:v for k,v in result.items() if k != "children"}), flush=True)
