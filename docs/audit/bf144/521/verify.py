import gzip
import hashlib
import json
import math
import os
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '520'
runtime = root / 'runtime-wall-162-aligned-air'

def read(path):
    return json.loads(path.read_bytes())

def save(name, result):
    (root / name).write_text(json.dumps(result, indent=2) + '\n')

def identity(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

assert read(root / 'geometry.native.json.reader.report.json')['success']
request = read(root / 'geometry.request.json')['samples']
samples = read(root / 'geometry.native.json')['geometry_query_probe']['samples']
assert [row['input'] for row in samples] == request
assert [row['input']['name'] for row in samples] == ['walking', 'falling-0']
assert all(row['capsule_sweep']['hit'] for row in samples)
for row in samples:
    normal = row['capsule_sweep']['normal']
    assert normal['z'] == 0.0 and abs(normal['x'] - normal['y']) < 2e-16

trace = read(runtime / 'wall-trace.json')
assert trace['actual_position'] == trace['final_position']
assert trace['actual_velocity'] == trace['final_velocity']
hit, = trace['hits']
assert hit['name'] == 'AuditWalkOffBarrier' and 0 < hit['normal'][1] < .001
assert hit['movement'] == trace['air_movement'] and hit['origin'] == trace['departure']
assert hit['outgoing'] == trace['actual_velocity']
assert math.dist(hit['normal'], [-math.sqrt(.5), 0.0, -math.sqrt(.5)]) > 4e-5

functional = read(root / 'wall-162-aligned-air-functional.json')
assert functional['pass'] and not functional['issues'] and functional['images'] == []
assert read(runtime / 'process.report.json')['success']
for label in ('baseline', 'ablated', 'restored'):
    assert read(root / f'replay-compile-{label}.report.json')['success']
for label in ('baseline', 'restored'):
    assert read(root / f'replay-{label}.report.json')['success']
assert read(root / 'replay-ablated.report.json')['process_exit_code'] == 101

baseline = (root / 'replay-baseline.json').read_bytes()
assert baseline == (root / 'replay-restored.json').read_bytes()
replay = json.loads(baseline)
ablated = read(root / 'replay-ablated.json')
assert ablated['baseline'] == ablated['source_normal_only'] == replay['baseline'] == hit['outgoing']
assert ablated['source_distance_and_normal'] == replay['source_distance_only']
assert replay['target_remaining_s'] == hit['remaining_time']
assert replay['source_normal_used'][1] == 0
assert math.dist(replay['source_distance_only'], replay['baseline']) < 4e-7

old = read(previous / 'actual-comparison.json')
native = old['native']['wall-162']
assert old['target']['wall-162-aligned-air']['velocity'] == trace['actual_velocity']
residual_before = [a-b for a,b in zip(replay['baseline'], native['velocity'], strict=True)]
residual_after = [a-b for a,b in zip(replay['source_normal_only'], native['velocity'], strict=True)]
assert abs(residual_before[0]) > 2e-4 and abs(residual_after[0]) < 2e-6
assert abs(residual_after[2]) < abs(residual_before[2])
assert abs(residual_after[1]) < abs(residual_before[1])
air_time_delta = trace['air_time'] - native['air_time']
assert air_time_delta > 0 and abs(residual_after[1] + air_time_delta * 9.81) < 4e-8
direct_native_hit = samples[1]['capsule_sweep']['distance_cm'] / 100
assert .00014 < direct_native_hit - hit['hit_distance'] < .00015

decoded = {}
for name in ('parameters.json', 'report.json'):
    path = runtime / name
    archive = path.with_suffix('.json.gz')
    data = path.read_bytes() if path.exists() else gzip.decompress(archive.read_bytes())
    archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = identity(data)
    if name == 'parameters.json':
        assert data == gzip.decompress((previous / runtime.name / archive.name).read_bytes())
    if path.exists():
        path.unlink()

ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
cmc = 'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
reader = 'tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(93, 93), (5054, 5134), (5257, 5287)]),
    (reader, Path(reader), [(1337, 1404)]),
    ('src/als/character/movement/motion.rs', Path('src/als/character/movement/motion.rs'), [(2701, 2750)]),
    ('src/tnua_ext/motion.rs', Path('src/tnua_ext/motion.rs'), [(3, 21)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

paths = {row['path'] for row in read(previous / 'summary.json')['external_source_files']}
paths.update(str(path.relative_to(Path.cwd())) for path in (
    previous / 'observer.py', previous / 'align_walking.rs', previous / 'actual-comparison.json',
    previous / 'wall.native.json.gz', previous / runtime.name / 'parameters.json.gz',
))
save('summary.json', dict(
    gate='PASS_FALLING_WALL_SWEEP_NORMAL_AND_LIVE_SLIDE_ABLATION', ordinal=49,
    findings=['BF144-D290'], new_differences=1, total_differences=287,
    actual_native_velocity_mps=native['velocity'], native_air_time_s=native['air_time'],
    target_air_time_s=trace['air_time'], air_time_delta_s=air_time_delta,
    previous_target_air_time_estimate_from_velocity_s=old['target']['wall-162-aligned-air']['air_time'],
    normal_control=replay, residual_before_mps=residual_before, residual_after_mps=residual_after,
    target_hit_distance_m=hit['hit_distance'], common_native_hit_distance_m=direct_native_hit,
    native_hit_normals=[row['capsule_sweep']['normal'] for row in samples],
    target_falling_normal=hit['normal'], previous_parameters_byte_exact=True, ablation_exit=101,
    restored_replay_byte_exact=True, decoded=decoded, screenshots=0, performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(paths)]))
print('Verified direct native and live wall sweeps, single-factor ablation and restored replay', flush=True)
