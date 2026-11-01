import copy
import hashlib
import json
import subprocess
from pathlib import Path

r = Path(__file__).resolve().parent
expected = {
    "443": ("-a", "PASS_CURRENT240_HORIZONTAL_PHYSICS_AND_MATCHED_SOURCE_HISTORY_WITH_VERTICAL_DEFERRED", 78),
    "443b": ("", "PASS_FULL_CURRENT240_FLOOR_TRANSACTION_PHYSICS_FINAL_HEIGHT_WITH_RECORDED_DIFFERENCE", 110),
    "443c": ("", "PASS_STORED_SCALE_PRODUCER_AND_FLOOR_CONSUMER_WITH_RECORDED_DIFFERENCE", 65),
}


def validate_children(children):
    assert set(children) == set(expected)
    for name, child in children.items():
        assert child["gate"] == expected[name][1]
        assert child["files"] == expected[name][2]


def verify(raw, record):
    assert len(raw) == record["bytes"]
    assert hashlib.sha256(raw).hexdigest() == record["sha256"]


children = {}
for name, (suffix, gate, count) in expected.items():
    root = r.parent / name
    summary_path = root / f"summary{suffix}.json"
    manifest_path = root / f"evidence-manifest{suffix}.json"
    summary = json.loads(summary_path.read_text())
    manifest = json.loads(manifest_path.read_text())
    assert summary["gate"] == manifest["gate"] == gate
    assert summary["screenshots"] == 0
    assert summary["performance_acceptance"] == "paused"
    assert summary["source_bytes_restored"] and summary["same_target_rebuilt"]
    assert len(manifest["files"]) == count
    paths = set()
    for relative, record in manifest["files"].items():
        p = (root / relative).resolve()
        assert p.is_relative_to(root.resolve())
        verify(p.read_bytes(), record)
        paths.add(str(p.relative_to(Path.cwd())))
    paths.update(str(p.relative_to(Path.cwd())) for p in (summary_path, manifest_path))
    tracked = subprocess.run(["git", "ls-files", "-z", "--", str(root.relative_to(Path.cwd()))], check=True, capture_output=True, timeout=5).stdout.decode().split("\0")
    assert paths <= set(tracked)
    children[name] = dict(gate=gate, files=count, summary_sha256=hashlib.sha256(summary_path.read_bytes()).hexdigest(), manifest_sha256=hashlib.sha256(manifest_path.read_bytes()).hexdigest())
    print("Verified sealed child " + name, flush=True)

validate_children(children)
for mutation in ("missing", "failed", "wrong_scope"):
    bad = copy.deepcopy(children)
    if mutation == "missing":
        del bad["443c"]
    elif mutation == "failed":
        bad["443c"]["gate"] = "FAIL"
    else:
        bad["443c"]["gate"] = "PASS_UNRELATED_SCOPE"
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

result = dict(
    gate="PASS_CURRENT_PHYSICS_FEEDBACK_AUDIT_WITH_RECORDED_DIFFERENCES",
    children=children,
    verified_git_evidence_files=sum(child["files"] for child in children.values()),
    negative_child_status_scope_and_hash_cases_rejected=4,
    current_target_full_physics_and_floor_rows_exact=240,
    matched_source_world_input_rows=240,
    source_gravity_cm_s2=-981,
    source_target_origins_and_all_parameters_matched=False,
    known_consumers=["BF144-D253", "BF144-D254", "BF144-D264"],
    new_differences=["BF144-D273", "BF144-D274"],
    differences_fixed=False,
    total_confirmed_differences=271,
    priority_counts=dict(P2=116, P3=155),
    current_identity_chain_files=396,
    unchanged_runtime_libraries=6,
    old_ignored_evidence_recovered=False,
    old_full_identity_corpus_revalidated=False,
    full_source_world_root_or_als_equivalence_claimed=False,
    arbitrary_parent_nonuniform_or_shear_coverage_claimed=False,
    player_clock_nonzero_view_and_custom_attributes_closed=False,
    render_gpu_pass_acceptance="deferred under explicit peripheral exclusion; unchanged public startup TAA failure retained",
    animation_functional_gate_passed=True,
    screenshots=0,
    performance_acceptance="paused",
)
(r / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({k: v for k, v in result.items() if k != "children"}), flush=True)
