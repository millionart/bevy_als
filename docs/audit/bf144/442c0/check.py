import copy
import gzip
import hashlib
import json
import sys
from pathlib import Path

r = Path(__file__).resolve().parent
sys.path.insert(0, str(Path("tools/tests").resolve()))
import test_character_player_view_probe as schema


def load(name):
    p = r / name
    if not p.exists():
        p = p.with_suffix(p.suffix + ".gz")
    with gzip.open(p, "rt") if p.suffix == ".gz" else p.open() as stream:
        return json.load(stream)


before = load("before.native.json")["character_animation_probe"]
baseline = load("baseline.native.json")
restored = load("restored.native.json")
assert schema.validate(baseline) == schema.validate(restored)
assert baseline["character_animation_probe"] == restored["character_animation_probe"]
stripped = copy.deepcopy(baseline["character_animation_probe"])
for case in stripped["cases"]:
    del case["initialization"]["controller_input"]["player_view"]
    for frame in case["frames"]:
        for key in ("controller_before_tick", "controller_input"):
            del frame[key]["player_view"]
assert stripped == before
try:
    schema.validate(load("ablated.native.json"))
except AssertionError:
    pass
else:
    raise AssertionError("Removed getter was not detected")
legacy_mini = load("ablated-legacy.native.json")["character_animation_probe"]
assert "player_view" in legacy_mini["cases"][0]["initialization"]["controller_input"]
for label, code in (("invalid", 7), ("ablated-invalid", 0), ("restored-invalid", 7)):
    report = load(label + ".native.json.reader.report.json")
    assert report["exit_reason"] == "completed" and report["process_exit_code"] == code
with gzip.open("docs/audit/bf144/441b/character.native.json.gz", "rt") as stream:
    old = json.load(stream)["character_animation_probe"]
legacy = load("legacy240.native.json")["character_animation_probe"]
assert sum(len(c["frames"]) for c in legacy["cases"]) == 240
assert old == legacy
source = Path("tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp")
assert hashlib.sha256(source.read_bytes()).hexdigest() == load("reader-final-sha256.json")["sha256"]
result = dict(
    gate="PASS_OPTIONAL_NATIVE_PLAYER_VIEW_CAPTURE_AND_LEGACY240_COMPATIBILITY",
    native=schema.validate(restored), self_test=schema.self_test(),
    entire_old_character_probe_unchanged=True, entire_legacy240_character_probe_unchanged=True,
    entire_new_character_probe_restored_exactly=True, source_bytes_restored=True,
    ablations=["getter_removed", "type_guard_removed", "default_off_guard_removed"],
    native_updated_cache_branch_claimed=False,
    enclosing_asset_graph_guid_equality_claimed=False,
    diagnostic_only=True, screenshots=0, performance_acceptance="paused")
(r / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result), flush=True)
