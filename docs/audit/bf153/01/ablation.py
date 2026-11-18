import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from tools.tests.test_platform_tools import PlatformToolsTest


def add_target_override(root):
    path = root / "run_manual.ps1"
    source = path.read_text()
    path.write_text('$env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot "target_game"\n' + source)


def add_symbol_cleanup(root):
    path = root / "run_manual.ps1"
    source = path.read_text()
    needle = "    $status = $LASTEXITCODE\n"
    cleanup = (
        '    if ($IsWindows -and (Test-Path (Join-Path $env:CARGO_TARGET_DIR "play"))) {\n'
        '        Get-ChildItem (Join-Path $env:CARGO_TARGET_DIR "play") -Filter *.pdb -Recurse -File | Remove-Item -Force\n'
        '    }\n'
    )
    assert source.count(needle) == 1
    path.write_text(source.replace(needle, needle + cleanup))


def restore_old_worktree(root):
    source = subprocess.check_output(["git", "show", "6219f761:tools/new-worktree.ps1"], cwd=ROOT, timeout=10)
    (root / "tools/new-worktree.ps1").write_bytes(source)


def run_case(name, mutation=None):
    test = PlatformToolsTest(name)
    setup = test.setUp

    def prepare():
        setup()
        if mutation:
            mutation(test.root)

    test.setUp = prepare
    result = unittest.TextTestRunner(stream=io.StringIO()).run(unittest.TestSuite([test]))
    return len(result.failures), len(result.errors), result.wasSuccessful()


files = [ROOT / "run_manual.ps1", ROOT / "tools/new-worktree.ps1"]
before = [hashlib.sha256(path.read_bytes()).hexdigest() for path in files]
controls = [
    ("target_override", "test_manual_launcher_keeps_target_profile_jobs_and_exit", add_target_override, 2),
    ("symbol_cleanup", "test_manual_launcher_preserves_windows_debug_symbols", add_symbol_cleanup, 1),
    ("second_cache", "test_worktree_copies_caches_and_mtimes_without_incremental", restore_old_worktree, 1),
]
results = []
for name, test, mutation, failures in controls:
    altered = run_case(test, mutation)
    restored = run_case(test)
    assert altered == (failures, 0, False), (name, altered)
    assert restored == (0, 0, True), (name, restored)
    results.append({"control": name, "test": test, "expected_failures": failures, "control_failed": True, "original_passed": True})
assert before == [hashlib.sha256(path.read_bytes()).hexdigest() for path in files]
print(json.dumps({"controls": results, "source_bytes_unchanged": True}, indent=2))
