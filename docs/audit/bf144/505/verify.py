import gzip
import hashlib
import json
import math
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def fields(values):
    return {value['name']: value for value in values}

def save(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

init = '74f5c4c5-443b-85dc-123b-5f80f77250f5'
refresh = '18c00b87-478a-29b9-7505-1daa6d305742'
graph = load('grounded.native.json')['graphs']
outer = next(item for item in graph if item['name'] == 'AnimGraph')
nodes = {node['node_guid']: node for node in outer['nodes']}
for guid, call, site in ((init, 'InitializeGrounded', 'OnBecomeRelevant'), (refresh, 'RefreshGrounded', 'OnUpdate')):
    node = nodes[guid]
    props = fields(node['properties'])
    assert fields(props['Node']['fields'])['CallSite']['exported_text'] == site
    prototype = props['CallFunctionPrototype']['exported_text'].split("'")[1]
    bound = next(child for item in graph for child in item['nodes'] if child['object_path'] == prototype)
    ref = fields(bound['properties'])['FunctionReference']
    assert fields(ref['fields'])['MemberName']['exported_text'] == call
assert fields(nodes[init]['pins'])['Source']['linked_to'][0]['node_guid'] == refresh
locomotion = load('locomotion.native.json')['graphs']
for name, expected in [('Grounded', 1), ('Land Movement', 2), ('Land', 0)]:
    subgraph = next(item for item in locomotion if item['name'] == name and ':AnimGraph.' in item['object_path'])
    assert sum(node['class_path'].endswith('.AnimGraphNode_UseCachedPose') for node in subgraph['nodes']) == expected

for name in ('grounded', 'locomotion', 'landing'):
    assert load(f'{name}.native.json.reader.report.json')['success']
print('Verified fresh Grounded and Locomotion native asset bindings', flush=True)
native = load('landing.native.json')['character_animation_probe']['cases'][0]['frames']
native_rows = []
for row in native:
    s = fields(row['source_state_after_tick'])
    b = fields(fields(s['GroundedState']['fields'])['VelocityBlend']['fields'])
    l = fields(s['LocomotionState']['fields'])
    x, y, z = (value['numeric_value'] for value in l['VelocityWorldSpace']['fields'])
    qx, qy, qz, qw = (value['numeric_value'] for value in l['RotationQuaternionWorldSpace']['fields'])
    amounts = [b[direction + 'Amount']['numeric_value'] for direction in ('Forward', 'Backward', 'Left', 'Right')]
    ground = next(item for item in row['linked_instances'] if item['class'].endswith('.AB_Als_Grounded_C'))
    loc = next(item for item in row['linked_instances'] if item['class'].endswith('.AB_Als_Locomotion_C'))
    main = next(item for item in row['linked_instances'] if item['main'])
    state = next(item['state'] for item in loc['machines'] if item['machine_class_index'] == 0)
    goal = None
    if row['frame'] in (25, 51):
        assert (z, qx, qy) == (0, 0, 0)
        tx, ty = 2 * qz * y, -2 * qz * x
        vx, vy = f32((x + qw * tx) + qz * ty), f32((y + qw * ty) - qz * tx)
        inv = f32(1 / f32(math.sqrt(f32(f32(vx * vx) + f32(vy * vy)))))
        nx, ny = f32(vx * inv), f32(vy * inv)
        inv = f32(1 / f32(abs(nx) + abs(ny)))
        gx, gy = f32(nx * inv), f32(ny * inv)
        goal = [max(gx, 0), max(-gx, 0), max(-gy, 0), max(gy, 0)]
        assert goal == amounts
    assert b['bInitializationRequired']['exported_text'] == 'False'
    native_rows.append(dict(frame=row['frame'], state=state, grounded_update_counter=ground['update_counter'],
                            movement=row['movement_after_tick']['movement_mode_name'],
                            tag=fields(s['LocomotionMode']['fields'])['TagName']['exported_text'],
                            amounts=amounts, recomputed_snap=goal,
                            roll_sequences=[segment['sequence'] for montage in main['montage_evaluation']
                                            for track in montage['tracks'] for segment in track['segments']
                                            if '/Roll/' in segment['sequence']]))
assert [row['state'] for row in native_rows[24:27]] == ['Fall', 'Land Movement', 'Land Movement']
assert native_rows[24]['movement'] == 'MOVE_Falling' and native_rows[25]['movement'] == 'MOVE_Walking'
assert all(row['amounts'] == [0, 0, 0, 0] for row in native_rows[:25])
assert native_rows[25]['roll_sequences'] and native_rows[51]['state'] == 'Grounded'
assert all(row['amounts'] == native_rows[25]['amounts'] for row in native_rows[26:51])
assert all(row['grounded_update_counter'] == native_rows[26]['grounded_update_counter'] for row in native_rows[27:51])
save('native-landing.json', native_rows)
print('Verified native landing snap and Roll interruption', flush=True)

assert raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
runs = {}
for label in ('baseline', 'no-initialize', 'restored'):
    functional = load(f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and functional['validation_scope'] == 'functional'
    for report in (f'{label}-functional-process.json', f'compile-{label}.report.json',
                   f'runtime-{label}/process.report.json'):
        assert load(report)['success']
    runtime = load(f'runtime-{label}/parameters.json')
    assert len(runtime['before']) == len(runtime['after']) == 56
    runs[label] = runtime
    previous = None
    for before, after in zip(runtime['before'], runtime['after'], strict=True):
        ordinal = after['ordinal']
        a, b = before['velocity_blend'], after['velocity_blend']
        assert before['ordinal'] == ordinal and after['graph_error'] is None
        assert a['instance_id'] == b['instance_id'] == 7
        assert a['required'] is b['required'] is False
        assert (a['velocity'], a['rotation'], a['delta']) == (b['velocity'], b['rotation'], b['delta'])
        if previous is not None:
            assert a['amounts'] == previous['amounts']
        if 34 <= ordinal <= 58:
            assert after['gameplay'] == 'Air' and after['graph_tag'].endswith('.InAir')
            assert b['calls'] == [] and a['write_frame'] == b['write_frame'] == 113
            assert a['amounts'] == b['amounts']
        else:
            assert after['gameplay'] == 'Grounded' and after['graph_tag'].endswith('.Grounded')
            assert [call[0] for call in b['calls']] == [refresh, init]
            assert b['write_frame'] == after['animation_frame']
            candidates = a['candidates']
            if ordinal == 59:
                assert candidates['snap'] != candidates['damp']
                assert b['amounts'] == candidates['damp' if label == 'no-initialize' else 'snap']
            else:
                assert b['amounts'] == candidates['damp']
        previous = b
for boundary in ('before', 'after'):
    reference, negative = runs['baseline'][boundary], runs['no-initialize'][boundary]
    assert [row for row in reference if row['ordinal'] < 59] == [row for row in negative if row['ordinal'] < 59]
    for a, b in zip(reference, negative, strict=True):
        assert {k:v for k,v in a.items() if k != 'velocity_blend'} == {k:v for k,v in b.items() if k != 'velocity_blend'}
        fields_to_match = ('calls', 'delta', 'instance_id', 'required', 'rotation', 'velocity', 'write_frame')
        assert all(a['velocity_blend'][key] == b['velocity_blend'][key] for key in fields_to_match)
source = root / 'velocity_observer.rs'
digest = hashlib.sha256(source.read_bytes()).hexdigest()
backup = Path('tools/tmp/bf144/505/velocity_observer.final.rs')
if backup.exists():
    assert source.read_bytes() == backup.read_bytes()
save('final-source.json', dict(sha256=digest))
print('Verified 56 pre/post frames in each of the three real Playground runs', flush=True)

sources = []
local = json.loads(Path('tools/env.local.json').read_text())
als = Path(local['ue_project']).parent / 'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
for key, path, ranges in (
    ('source:Private/AlsAnimationInstance.cpp', als/'Private/AlsAnimationInstance.cpp', [(798, 827), (850, 895)]),
    ('source:Private/AlsLinkedAnimationInstance.cpp', als/'Private/AlsLinkedAnimationInstance.cpp', [(82, 95)]),
    ('Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_CallFunction.cpp', ue/'Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_CallFunction.cpp', [(21, 68)]),
    ('src/als/character/animation_instances/source_native_refresh.rs', Path('src/als/character/animation_instances/source_native_refresh.rs'), [(135, 186)]),
    ('src/animation_graph_ext/source_instance.rs', Path('src/animation_graph_ext/source_instance.rs'), [(200, 247)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    sources.append(dict(key=key, sha256=hashlib.sha256(data).hexdigest(),
                        excerpts=[dict(start=a, end=b, text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json', sources)

decoded = {}
for name in ['grounded.native.json', 'locomotion.native.json', 'landing.native.json'] + [
    f'runtime-{label}/{file}.json' for label in runs for file in ('report', 'parameters')
]:
    data = raw(name)
    path = root / name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name, flush=True)
external = [
    root.parent/'484/playground_observed.rs', root.parent/'484/foot_observer.rs',
    root.parent/'499/replay.rs', root.parent/'504/summary.json',
    root.parent/'491/reader-migration.json', root.parent/'441c/library-identities.json',
    Path('assets/ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph.animgraph.ron')
]
files = [dict(path=str(p.resolve().relative_to(Path.cwd())), bytes=p.stat().st_size,
              sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in external]
save('summary.json', dict(
    gate='PASS_GROUNDED_VELOCITY_BLEND_RELEVANCE_AND_LANDING_RESET',
    target_air_ordinals=[34, 58], target_landing_ordinal=59, target_damping_resumed=60,
    native_moving_landing_frame=25, native_post_roll_grounded_frame=51,
    native_roll_interruption_requires_matched_target_case=True,
    restored_parameters_exact=True, screenshots=0, performance_acceptance='paused',
    new_differences=0, total_differences=279, decoded=decoded, external_source_files=files))
