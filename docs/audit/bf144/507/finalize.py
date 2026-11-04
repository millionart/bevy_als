import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def save(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')

runs = {}
for label in ('edge', 'native-hold', 'restored'):
    for report in (f'{label}-functional-process.json', f'runtime-{label}/process.report.json'):
        assert load(report)['success'], report
    check = load(f'{label}-functional.json')
    assert check['pass'] and check['images'] == [] and check['validation_scope'] == 'functional'
    d = load(f'runtime-{label}/parameters.json')
    runs[label] = {r['ordinal']:r for r in d['after']}
assert load('compile-edge.report.json')['success']
assert raw('runtime-edge/parameters.json') == raw('runtime-restored/parameters.json')
base, held = runs['edge'], runs['native-hold']
focused = next(s['frames'] for s in load('replay-baseline.json') if s['name'] == 'fade-edge-5')
for offset in range(30):
    row = base[59+offset]
    assert row['roll']['time'] == focused[offset]['time']
    assert row['post_locomotion']['weight'] == focused[offset]['weight']
    assert row['velocity_blend']['delta'] == focused[offset]['delta']
assert all(base[i] == held[i] for i in base if i < 85)
assert all(base[85][key] == held[85][key] for key in ('roll', 'gameplay', 'graph_tag', 'input', 'position', 'velocity', 'animation_frame'))
assert base[85]['post_locomotion']['weight'] == 0.9657064080238342
assert held[85]['post_locomotion']['weight'] == 1
assert len(base[85]['velocity_blend']['calls']) == 2
assert base[85]['velocity_blend']['write_frame'] == base[85]['animation_frame']
assert held[85]['velocity_blend']['calls'] == []
assert held[85]['velocity_blend']['write_frame'] == held[84]['velocity_blend']['write_frame']
assert held[85]['velocity_blend']['amounts'] == held[84]['velocity_blend']['amounts']
assert base[85]['velocity_blend']['amounts'] != held[85]['velocity_blend']['amounts']
assert base[86]['post_locomotion'] == held[86]['post_locomotion']
assert len(held[86]['velocity_blend']['calls']) == 2
assert held[86]['velocity_blend']['write_frame'] == held[86]['animation_frame']
save('actual-boundary.json', {label: [run[i] for i in range(83, 88)] for label, run in runs.items()})
print('Verified actual Grounded resumes one frame early and the native-weight hold restores it', flush=True)

variants = ('no-branching', 'elapsed-trigger', 'elapsed-duration', 'no-alpha-history', 'reverse-curve')
negative = {}
for mode in variants:
    assert not load(f'verify-{mode}.report.json')['success']
    negative[mode] = load(f'negative-{mode}.json')['total_mismatches']
    assert negative[mode] > 0
assert load('verify.report.json')['success']

facts = []
engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
runtime = engine / 'Engine/Source/Runtime'
for key, path, ranges in (
    ('Engine/Source/Runtime/Engine/Private/Animation/AnimMontage.cpp',
     runtime/'Engine/Private/Animation/AnimMontage.cpp', [(2240, 2256), (2280, 2338), (2468, 2488), (2622, 2640), (2714, 2748)]),
    ('Engine/Source/Runtime/Engine/Private/AlphaBlend.cpp',
     runtime/'Engine/Private/AlphaBlend.cpp', [(192, 248), (287, 294)]),
    ('Engine/Source/Runtime/Engine/Private/Animation/AnimInstance.cpp',
     runtime/'Engine/Private/Animation/AnimInstance.cpp', [(561, 578)]),
    ('src/als/character/actions/roll.rs', Path('src/als/character/actions/roll.rs'), [(9, 15), (87, 97), (134, 147)]),
    ('src/als/character/actions/roll_drive.rs', Path('src/als/character/actions/roll_drive.rs'), [(2017, 2028), (2238, 2243)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, sha256=hashlib.sha256(data).hexdigest(),
                      excerpts=[dict(start=a, end=b, text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json', facts)

decoded = {}
for name in [f'runtime-{label}/{file}.json' for label in runs for file in ('report', 'parameters')]:
    data = raw(name)
    path = root / name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name, flush=True)
paths = [parent / name for name in (
    '484/playground_observed.rs', '484/foot_observer.rs', '505/observer.py',
    '505/velocity_observer.rs', '506/observer.py', '506/slot_observer.rs',
    '506/native-slot.json', '506/runtime-baseline/parameters.json.gz',
    '506/summary.json', '491/reader-migration.json', '441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())), bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
comparison = load('comparison.json')
save('summary.json', dict(
    gate='PASS_ROLL_FADE_CLOCK_AND_GROUNDED_REENTRY_D118',
    samples=len(comparison), native_replay_frames=sum(s['frames'] for s in comparison),
    trigger_counterexamples=['fade-edge-5', 'fade-edge-6'],
    scoped_to_existing_difference='BF144-D118',
    actual_early_reentry_ordinal=85, controlled_reentry_ordinal=86,
    native_reentry_offset=27, target_reentry_offset=26,
    ablation_mismatches=negative, restored_parameters_exact=True,
    screenshots=0, performance_acceptance='paused',
    new_differences=0, total_differences=279, decoded=decoded, external_source_files=external))
