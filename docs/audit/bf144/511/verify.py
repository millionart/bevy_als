import gzip
import hashlib
import json
import math
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def raw(name):
    path = root/name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def save(name, value):
    (root/name).write_text(json.dumps(value, indent=2) + '\n')

def props(rows):
    return {row['name']: row for row in rows}

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def damp(previous, goal, delta):
    x = f32(f32(f32(math.log(2)) / f32(f32(0.2) + f32(1e-8))) * delta)
    first = f32(f32(1.00746054) * x)
    second = f32(f32(f32(0.45053901) * x) * x)
    third = f32(f32(f32(f32(0.25724632) * x) * x) * x)
    alpha = f32(1 - f32(1 / f32(f32(f32(1 + first) + second) + third)))
    return [f32(a + f32(alpha * f32(b-a))) for a,b in zip(previous,goal,strict=True)]

position = '6c1fffa0-d2ee-5aa2-99f8-54338425771f'
blend = '34f57700-443d-7c77-5457-2cb100b5b922'
add = '7c742ec2-4a57-89f7-5699-fd9f6869c86c'
sprint = '399f03d7-4ff5-edce-96c3-35a0f7a7c922'
assert load('standing.native.json.reader.report.json')['success']
assert load('compile-observed.report.json')['success']
assert not load('compile-cache-value-lifetime.report.json')['success']
assert load('runtime-uncompiled/process.report.json')['exit_reason']=='start_failed'
native = load('standing.native.json')
graph = next(g for g in native['graphs'] if g['name']=='AnimGraph')
nodes = {n['node_guid']:n for n in graph['nodes']}
for guid, expected in (
    (blend, ('GetParent.LeanState.ForwardAmount', 'GetParent.LeanState.RightAmount')),
    (add, ('GetParent.PoseState.UnweightedGaitRunningAmount',)),
    (sprint, ('GetParent.StandingState.SprintAccelerationAmount',)),
):
    path = props(nodes[guid]['properties'])['Binding']['exported_text'].split("'")[1]
    obj = next(row for row in native['asset_objects'] if row['object_path']==path)
    bindings = props(obj['properties'])['PropertyBindings']['exported_text']
    assert all(binding in bindings for binding in expected)
assert props(nodes[add]['pins'])['Additive']['linked_to'][0]['node_guid'] == blend
del native
print('Verified native Standing Lean, additive and Sprint bindings', flush=True)

labels = ('baseline','cancel','braking-only','clamp-local','restored')
reports = {}
for label in labels:
    for name in (f'runtime-{label}/process.report.json', f'{label}-functional-process.json'):
        assert load(name)['success']
    functional = load(f'{label}-functional.json')
    assert functional['pass'] and functional['validation_scope']=='functional' and functional['images']==[]
    reports[label] = load(f'runtime-{label}/parameters.json')
assert raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
def compact(row):
    return {k:v for k,v in row.items() if k not in ('standing_consumer','rig_poses')}
for label in ('baseline','cancel','braking-only','restored'):
    old = json.loads(gzip.decompress((parent/f'510/runtime-{label}/parameters.json.gz').read_bytes()))
    assert {side:[compact(row) for row in rows] for side,rows in reports[label].items()} == old

def table(label, side='after'):
    return {row['ordinal']:row for row in reports[label][side]}

def observed(row):
    state = row['standing_consumer']
    assert state['sprinting']==0 and state['sprint_acceleration']==0
    nodes = {row['guid']:row for row in state['nodes']}
    assert sprint not in nodes
    assert set(nodes) == {blend,position,add}
    return nodes

def changes(a,b):
    assert len(a['bones'])==len(b['bones']) and len(a['bones']) in (79,80)
    assert a['skeleton_asset']==b['skeleton_asset'] and a['timestamp']==b['timestamp']
    assert [x['name'] for x in a['bones']] == [x['name'] for x in b['bones']]
    changed = [x['name'] for x,y in zip(a['bones'],b['bones'],strict=True) if x!=y]
    assert None not in changed
    return changed

def compare(left,right,frames):
    before, after = table(left),table(right)
    result = {}
    for frame in frames:
        a,b = before[frame],after[frame]
        assert a['graph_error'] is None and b['graph_error'] is None
        for key in ('gameplay','graph_tag','position','velocity','input'):
            assert a[key] == b[key], (frame,key)
        nodes_a,nodes_b = observed(a),observed(b)
        result[frame] = dict(
            coordinates=[nodes_a[position]['coordinates'],nodes_b[position]['coordinates']],
            blend=changes(nodes_a[blend]['pose'],nodes_b[blend]['pose']),
            additive=changes(nodes_a[add]['pose'],nodes_b[add]['pose']),
            pose={k:changes(v['pose'],b['rig_poses'][k]['pose']) for k,v in a['rig_poses'].items()},
        )
    return result

branch = compare('cancel','braking-only',(20,21))
idle = [table(label)[19] for label in ('cancel','braking-only')]
assert idle[0] == idle[1] and idle[0]['standing_consumer']['nodes'] == []
for ordinal in (20,21):
    assert branch[ordinal]['coordinates'] == [table(label)[ordinal]['lean']['amounts'][::-1] for label in ('cancel','braking-only')]
assert [len(branch[20][key]) for key in ('blend','additive')] == [68,59]
assert len(branch[20]['pose']['root_result']) == 72

reference = table('baseline')
control = table('clamp-local')
kernel = json.loads((parent/'510/replay-baseline.json').read_bytes())
retained = []
for row in kernel['target']:
    if row['actual']==row['clamp_local']:
        continue
    ordinal = row['frame']
    old = table('baseline','before')[ordinal]['lean']['amounts']
    delta = reference[ordinal]['velocity_blend']['delta']
    assert damp(old,row['actual'],delta)==reference[ordinal]['lean']['amounts']
    moved = damp(old,row['clamp_local'],delta)
    if moved != reference[ordinal]['lean']['amounts']:
        retained.append(ordinal)
    if ordinal==61:
        assert moved[::-1] == observed(control[ordinal])[position]['coordinates']
        assert reference[ordinal]['lean']['amounts'] == control[ordinal]['lean']['amounts']
        assert compact(reference[ordinal]) == compact(control[ordinal])
assert retained == [61,63]
clamp = compare('baseline','clamp-local',(60,61,62))
assert not clamp[60]['pose']['root_result']
assert len(clamp[61]['blend'])==69 and len(clamp[61]['additive'])==43
assert len(clamp[61]['pose']['root_result'])==71
assert clamp[62]['coordinates'][0]==clamp[62]['coordinates'][1]
assert not clamp[62]['blend'] and not clamp[62]['additive']
assert not clamp[62]['pose']['rig_source'] and len(clamp[62]['pose']['root_result'])==6
save('pose-impact.json',dict(branch=branch,local_clamp=clamp,retained_lean_frames=retained))
print('Verified real Lean-to-pose chains and exact no-control restoration', flush=True)

decoded = {}
for name in ['standing.native.json'] + [f'runtime-{label}/{file}.json'
        for label in labels for file in ('report','parameters')]:
    data = raw(name)
    path = root/name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name,flush=True)
env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for key,path,ranges in (
    ('source:Private/AlsAnimationInstance.cpp', als/'Private/AlsAnimationInstance.cpp',[(903,919)]),
    ('src/als/character/animation_instances/source_native_refresh.rs',Path('src/als/character/animation_instances/source_native_refresh.rs'),[(241,256)]),
    ('src/animation_graph_ext/source_pose_nodes.rs',Path('src/animation_graph_ext/source_pose_nodes.rs'),[(519,555)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=start,end=end,text='\n'.join(lines[start-1:end])+'\n') for start,end in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'consumer.rs').read_bytes()).hexdigest()))
paths = [parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs','510/controls.json',
    '510/replay-baseline.json','491/reader-migration.json','441c/library-identities.json')]
paths.extend(parent/f'510/runtime-{label}/parameters.json.gz' for label in ('baseline','cancel','braking-only','restored'))
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_D285_D286_STANDING_LEAN_POSE_CONSUMERS', known_differences=['BF144-D285','BF144-D286'],
    new_differences=0,total_differences=283,
    d285_query=20,d285_root_changed_bones=72,d286_query=61,d286_root_changed_bones=71,
    d286_next_query_residual_bones=6,local_clamp_retained_lean_frames=retained,
    sprint_consumer_active=False,screenshots=0,performance_acceptance='paused',decoded=decoded,
    external_source_files=external))
