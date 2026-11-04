import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


assert not read(root / 'first-compile.report.json')['success']
for name in ('prepare.report', 'compile-gjk.report', 'probe.report'):
    assert read(root / f'{name}.json')['success']
assert all(value == 'PASS' for value in read(root / 'watchdog-selftest.json').values())
first = read(root / 'first-probe.json')
assert len(first) == 8
assert sum(row['public_toi'] == row['expected'] for row in first) == 7
original_short, = [row for row in first if row['name'] == 'brake-short']
assert original_short['public_toi'] != original_short['expected']
fixed = read(root / 'before-ablation.json')
ablated = read(root / 'ablation.json')
rows = read(root / 'probe.json')
inputs = read(root / 'inputs.json')
assert len(rows) == len(fixed) == len(inputs) == len(ablated) == 8
native = {}
for item, prefix in (('577', 'landing-'), ('578', 'brake-')):
    result = read(root.parent / item / 'geometry.native.json')
    for row in result['geometry_query_probe']['samples']:
        name = row['input']['name']
        if item == '578':
            name = name.removeprefix('1-')
        native[prefix+name] = row['capsule_sweep']['distance_cm']/100

summary = {}
for row, case, baseline, prior in zip(rows, inputs, fixed, ablated):
    name = row['name']
    assert name == case['name'] == baseline['name'] == prior['name']
    assert row['public_toi'] == row['replay_toi'] == row['expected'] == case['expected']
    assert row['public_normal'] == row['replay_normal']
    assert row['public_toi'] == baseline['public_toi'] == prior['public_toi']
    for key in ('trace', 'ablated_trace'):
        assert [event for event in row[key] if event['event'] != 'simplex_update'] == prior[key]
    terminal = row['trace'][-1]
    projection = row['trace'][-2]
    assert terminal['event'] == 'non_decreasing_upper_bound'
    assert terminal['toi'] == row['replay_toi'] and terminal['toi'] > 0
    assert terminal['bound'] >= terminal['previous']
    assert projection['event'] == 'projection'
    assert projection['distance'] == terminal['bound'] > projection['tolerance']
    update = row['trace'][-3]
    assert update['event'] == 'simplex_update'
    assert update['before'] == update['after'] == 2
    rejected = name in ('landing-negative-actual', 'landing-negative-direction_control',
                        'brake-full', 'brake-short')
    assert update['added'] != rejected
    ablated_terminal = row['ablated_trace'][-1]
    assert ablated_terminal['event'] == ('inside_simplex' if name == 'landing-negative-direction_control'
                                        else 'distance_tolerance')
    assert row['ablated_toi'] == ablated_terminal['toi'] > row['public_toi']
    target_error = row['public_toi'] - native[name]
    ablation_error = row['ablated_toi'] - native[name]
    summary[name] = dict(native_toi=native[name], target_toi=row['public_toi'],
                         diagnostic_toi=row['ablated_toi'], terminal=terminal,
                         last_simplex_update=update, diagnostic_terminal=ablated_terminal,
                         error=target_error, diagnostic_error=ablation_error)

negative = summary['landing-negative-actual']
positive = summary['landing-positive-actual']
assert -.000501 < negative['error'] < -.000499
assert 8e-6 < negative['diagnostic_error'] < 10e-6
assert abs(positive['error']) < 3e-8 and positive['diagnostic_error'] > 2e-6
assert abs(summary['brake-full']['diagnostic_error']) < 1e-6
assert abs(summary['brake-short']['diagnostic_error']) < 1e-6

print('Verified eight live-query reproductions and their exact bound-return branches', flush=True)
paths = [root / name for name in ('probe.json', 'first-probe.json', 'before-ablation.json', 'ablation.json')]
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external = []
for name in ('577/runtime-positive/landing-geometry.json', '577/runtime-negative/landing-geometry.json',
             '577/geometry.native.json.gz', '578/runtime-baseline/brake-replay.json',
             '578/geometry.native.json.gz', '577/evidence-manifest.json', '578/evidence-manifest.json'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
result = dict(gate='PASS_LOCKED_GJK_BOUND_EXIT_REPRODUCES_D125', findings=['BF144-D125'],
              new_differences=0, total_differences=310, results=summary,
              decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
