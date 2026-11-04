import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
scratch = Path('tools/tmp/bf144/504')

def read(name):
    return json.loads((root / name).read_bytes())

def write(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')

def fingerprint(path, raw):
    return dict(path=str(path.resolve().relative_to(Path.cwd())), bytes=len(raw),
                sha256=hashlib.sha256(raw).hexdigest())

for label in ('baseline', 'widened'):
    functional = read(f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == []
    assert functional['validation_scope'] == 'functional'
    assert read(f'{label}-functional-process.json')['success']
    assert read(f'runtime-{label}/process.report.json')['success']
for report in ('prepare', 'observer', 'compile-observed', 'replay-baseline',
               'replay-restored', 'replay-compile-baseline',
               'replay-compile-no-widened-seed', 'replay-compile-restored'):
    assert read(report + '.report.json')['success'], report
for report in ('prepare-absent-idle-gait', 'replay-compile-explicit-default', 'replay-no-widened-seed'):
    assert not read(report + '.report.json')['success'], report

comparison = read('comparison.json')
assert comparison['input_released'] == 139
assert comparison['stationary'] == 146
assert comparison['first_persistent_root_equality'] == 149
assert comparison['first_persistent_pole_equality'] == 150
assert comparison['last_observed'] == 198
replay = read('replay-baseline.json')
assert (root / 'replay-baseline.json').read_bytes() == (root / 'replay-restored.json').read_bytes()
assert replay['mismatches'] == replay['previous_mismatches'] == 0
assert len(replay['rows']) == 192
for row in replay['rows']:
    query = row['query']
    left, right = row['outputs']
    assert (left == right) == (query >= 150)
    reset_left, reset_right = row['forced_reset']
    assert reset_left == reset_right and (reset_left is not None) == (query == 146)
negative = read('replay-no-widened-seed.json')
assert (negative['mismatches'], negative['previous_mismatches']) == (94, 96)
source = root / 'replay.rs'
assert source.read_bytes() == (scratch / 'replay.rs.final').read_bytes()
write('final-source.json', fingerprint(source, source.read_bytes()))

external = [parent / name for name in (
    '503/observer.py', '503/runtime-baseline/parameters.json.gz',
    '503/runtime-widened/parameters.json.gz', '503/summary.json',
    '502/replay-restored.json.gz', '498/observer.py', '484/playground_observed.rs',
    '484/foot_observer.rs', '491/reader-migration.json', '441c/library-identities.json')]
decoded = {}
for label in ('baseline', 'widened'):
    for filename in ('parameters.json', 'report.json'):
        path = root / f'runtime-{label}' / filename
        zipped = path.with_suffix('.json.gz')
        if path.exists():
            raw = path.read_bytes()
            zipped.write_bytes(gzip.compress(raw, mtime=0))
            assert gzip.decompress(zipped.read_bytes()) == raw
        else:
            raw = gzip.decompress(zipped.read_bytes())
        decoded[str(zipped.relative_to(root))] = fingerprint(path, raw)
    print(f'Validated archived runtime {label}', flush=True)

write('summary.json', dict(
    gate='PASS_D282_TAIL_AND_NATURAL_POLE_CONVERGENCE',
    **{key: value for key, value in comparison.items() if key != 'changed_bones'},
    replay_pairs=192, restored_mismatches=0,
    same_initial_seed_ablation_output_mismatches=94,
    same_initial_seed_ablation_previous_mismatches=96,
    rig_constructions_per_run=1, standalone_forced_reset_query=146,
    screenshots=0, performance_acceptance='paused', new_differences=0, total_differences=279,
    decoded=decoded,
    generated_observer_sha256=hashlib.sha256((scratch / 'playground_observed.rs').read_bytes()).hexdigest(),
    external_source_files=[fingerprint(path, path.read_bytes()) for path in external]))
