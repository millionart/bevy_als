import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '521'
labels = ('baseline', 'cleared', 'restored')

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

def save(name, result):
    (root / name).write_text(json.dumps(result, indent=2) + '\n')

def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')

records = {}
decoded = {}
for label in labels:
    runtime = root / f'runtime-{label}'
    assert read(root / f'compile-{label}.report.json')['success']
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    assert raw(runtime / 'wall-trace.json') == raw(previous / 'runtime-wall-162-aligned-air/wall-trace.json')
    records[label] = {
        f'{stage}-{frame}': load(label, f'contact-{stage}-{frame}')
        for frame in (49, 50) for stage in ('before', 'after')
    }
    for name in ('parameters.json', 'report.json'):
        path = runtime / name
        archive = path.with_suffix('.json.gz')
        data = raw(path)
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
        assert gzip.decompress(archive.read_bytes()) == data
        decoded[str(path.relative_to(root))] = identity(data)
        if name == 'parameters.json':
            assert data == raw(previous / 'runtime-wall-162-aligned-air' / name)
        if path.exists():
            path.unlink()

base = records['baseline']
removed = records['cleared']
restored = records['restored']
assert restored == base
for key in base:
    assert base[key] == removed[key] | ({'consumed_contact': True} if key == 'before-49' else {})
assert not removed['before-49']['consumed_contact']
for frame in (49, 50):
    before, after = base[f'before-{frame}'], base[f'after-{frame}']
    assert before['generated_contact'] == before['consumed_contact'] == (frame == 49)
    assert before['floor'] is None and before['manifolds'] == []
    assert before['requested_mode'] is None and before['raw_velocity'] == [0, 0, 0]
    assert before['mode'] == after['mode'] == 'Air' and not after['has_base'] and not after['stored_contact']
    assert before['planned_velocity'] == after['velocity'] == after['published']
    assert before['position'] == after['position']
assert base['after-49']['transitions'] == [['LocomotionState(Grounded)', 'LocomotionState(Air)']]
assert base['after-50']['transitions'] == []

prior_raw = root.parent / '520' / 'wall.native.json.gz'
prior_probe = json.loads(gzip.decompress(prior_raw.read_bytes()))['character_animation_probe']
case = next(row for row in prior_probe['cases'] if row['name'] == 'wall-162')
native = [case['frames'][frame]['movement_after_tick'] for frame in (49, 50)]
assert all(row['movement_mode_name'] == 'MOVE_Falling' and not row['walkable_floor'] for row in native)
assert read(previous / 'summary.json')['target_falling_normal'][1] < 1e-4

ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
cmc = 'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
math_header = 'Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h'
motion = 'src/als/character/movement/motion.rs'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(5047, 5083), (6983, 7016), (7374, 7442)]),
    (math_header, ue / math_header, [(128, 132)]),
    (motion, Path(motion), [(2574, 2581), (2723, 2744), (3979, 4008), (4070, 4124), (5274, 5297)]),
    ('src/als/settings/movement_constants.rs', Path('src/als/settings/movement_constants.rs'), [(1, 5)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

paths = {row['path'] for row in read(previous / 'summary.json')['external_source_files']}
paths.update(str(path.relative_to(Path.cwd())) for path in (
    prior_raw, previous / 'observer.py', previous / 'trace.rs', previous / 'summary.json',
    previous / 'geometry.native.json', previous / 'runtime-wall-162-aligned-air/wall-trace.json',
    previous / 'runtime-wall-162-aligned-air/parameters.json.gz',
))
save('summary.json', dict(
    gate='PASS_WALL_SWEEP_CONTACT_LIFETIME_WITHOUT_FLOOR', ordinals=[49, 50], new_differences=0,
    total_differences=287, related_difference='BF144-D290', rows=records, native=native,
    byte_exact_wall_trace_and_parameter_reports=True, restored_snapshots_exact=True,
    limitation='The observed wall has no eligible floor during contact consumption.',
    decoded=decoded, screenshots=0, performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(paths)]))
print('Verified wall-only contact removal, published movement and native falling modes', flush=True)
