import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(path):
    raw = gzip.decompress(path.read_bytes()) if path.suffix=='.gz' else path.read_bytes()
    return json.loads(raw)

def named(items,name):
    return next(item for item in items if item['name']==name)

def node(row,guid,pin='Pose'):
    values = [item for item in row['scale_nodes'] if item['guid'].startswith(guid) and item['pin']==pin]
    assert len(values)==1,(guid,len(values))
    return values[0]

def clean(row):
    scales = []
    for node in row['scale_nodes']:
        item = {k:v for k,v in node.items() if k not in ['gait','gait_blend']}
        item['inputs'] = sorted(item['inputs'],key=lambda edge:edge['pin'])
        scales.append(item)
    return {**row,'scale_nodes':scales}

target = read(root/'runtime-observed/parameters.json')
previous = read(parent/'497/runtime-observed/parameters.json.gz')
assert len(target)==len(previous)==16
assert list(map(clean,target))==list(map(clean,previous))
print('Verified the added gait observation preserves the existing actual trajectory',flush=True)

native = read(parent/'493/virtual-motion.native.json.gz')['character_animation_probe']['cases'][0]['frames']
rows = []
for query in [104,105,106]:
    raw = target[query-95]
    readback = target[query-95+1]
    source = native[query-1]
    native_consumer = native[query]
    pose = named(native_consumer['source_state_after_tick'],'PoseState')['fields']
    grounded = named(source['source_state_after_tick'],'GroundedState')['fields']
    velocity = named(grounded,'VelocityBlend')['fields']
    standing = next(value for value in source['linked_instances'] if '/AB_Als_Standing.' in value['class'])
    directions = []
    for name,identifier in [('forward','8c0d0f93'),('right','2b4c60ec')]:
        player = next(value for value in standing['asset_players'] if value['node_guid'].startswith(identifier))
        assert player['sync_read_buffer_tick_present'] and player['tick_weight']>0
        samples = player['blend_samples']
        assert {s['index'] for s in samples}=={0,2,3}
        assert all(s['pose_mode_at_observation']=='compressed' for s in samples)
        target_direction = node(raw,identifier,'pose')
        directions.append(dict(name=name,target=target_direction['gait_blend'],target_gait=target_direction['gait'],
                               native_samples=samples,native_direction_weight=player['tick_weight']))
    multi = node(raw,'41fd7093')
    assert multi['multi']['normalize'] and multi['multi']['scale']==1
    assert multi['multi']['desired'][1:3]==[0,0]
    for id in ['4c6fa15f','25575e9e','47121ad2','d17ea3bf']:
        assert node(raw,id)['gait']==multi['gait'],(query,id)
    target_root = raw['rig_snapshot']['upstream_context_curves']['PoseGait']
    assert target_root==multi['gait']
    assert raw['rig_snapshot']['upstream_context_curves']['PoseGrounded']==1
    assert source['curves']['PoseGrounded']==named(pose,'GroundedAmount')['numeric_value']==1
    assert named(pose,'GaitAmount')['numeric_value']==source['curves']['PoseGait']
    target_raw = node(readback,'7c742ec2')['lean']['raw']
    rows.append(dict(query=query,directions=directions,target_desired=multi['multi']['desired'],
                     native_desired=[named(velocity,f'{name}Amount')['numeric_value']
                                     for name in ['Forward','Backward','Left','Right']],
                     target_root_gait=target_root,native_root_gait=source['curves']['PoseGait'],
                     target_running_consumer=target_raw,
                     native_running_consumer=named(pose,'UnweightedGaitRunningAmount')['numeric_value']))
assert rows[0]['target_root_gait']==1.677905797958374
assert rows[0]['native_root_gait']==1.6779059171676636
assert rows[0]['target_desired']!=rows[0]['native_desired']
assert all(d['target_gait']==rows[0]['native_root_gait'] for d in rows[0]['directions'])

graph = Path('assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron')
lines = graph.read_text().splitlines()
for prefix in ['8c0d0f93','2b4c60ec']:
    line = next(x for x in lines if f'id: "{prefix}-' in x)
    for sample,gait in [(0,1.0),(2,2.0),(3,2.0)]:
        section = line.split(f'id: "sample_{sample}",',1)[1].split('(id: "sample_',1)[0]
        assert f'"PoseGait": (times: [0.0], values: [{gait}]' in section
(root/'evidence-input.json').write_text(json.dumps(dict(rows=rows,pose_gait=[1,1,2,2],graph=graph.as_posix()),indent=2)+'\n')
print('Qualified actual directional curves, movement weights and next-frame Lean consumption',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(351,375),(864,901),(1010,1017)]),
    (als,'Private/AlsCharacterMovementComponent.cpp',[(969,986)]),
    (engine,'Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_MultiWayBlend.cpp',[(49,91),(128,164)]),
    (Path('.'),'src/als/nodes/source_movement_nodes.rs',[(81,104),(111,137)]),
    (Path('.'),'src/animation_graph_ext/normalized_blendspace.rs',[(455,469),(513,527),(831,844)]),
    (Path('.'),'src/animation_graph_ext/source_pose_nodes.rs',[(967,985)]),
    (Path('.'),'src/als/character/animation_instances/standing_layer.rs',[(2301,2314)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')

weights = (parent/'497/replay.rs').read_text()
a = weights.index('fn point(')
b = weights.index('fn main()',a)
weights = weights[a:b].replace('vec![0, 1, 4]','vec![0, 2, 3]')
weights = weights.replace('Recorded Lean samples','Recorded WalkRun samples')
Path('tools/tmp/bf144/498/weights.rs').write_text(weights)
