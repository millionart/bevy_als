import gzip
import json
from pathlib import Path

parent = Path(__file__).resolve().parent.parent
out = parent/'485'
def load(path):
    with gzip.open(parent/path, 'rt') as stream:
        return json.load(stream)

def fields(obj):
    return {v['name']: v for v in obj['fields']}

def transform(value):
    return {key: [value[key][axis] for axis in axes]
            for key, axes in [('translation','xyz'),('rotation','xyzw'),('scale','xyz')]}

case = load('457/restored-final.native.json.gz')['character_animation_probe']['cases'][0]
frames = case['frames']
assert len(frames) == 318 and case['mesh_tick_after_character']
bones = frames[0]['pose_boundaries']['root_result']['pose']['bones']
names = [bone['name'] for bone in bones]
assert len(names) == len(set(names)) == 79
def mesh(frame):
    by_name = {bone['name']: bone['local'] for bone in frame['mesh_local_pose']}
    assert set(by_name) == set(names)
    return [transform(by_name[name]) for name in names]

previous = mesh(case['initialization'])
previous_raw = None
rows = []
for frame in frames:
    feet = fields(next(x for x in frame['source_state_after_tick'] if x['name']=='FeetState'))
    assert fields(frame['getter_after_tick'])['bUseFootIkBones']['exported_text'] == 'True'
    expected = []
    for side in ['Left','Right']:
        foot = fields(feet[side])
        expected.append([[value['numeric_value'] for value in foot[key]['fields']]
                         for key in ['TargetLocationWorldSpace','TargetRotationWorldSpace']])
    current = mesh(frame)
    raw_bones = frame['pose_boundaries']['root_result']['pose']['bones']
    assert [b['name'] for b in raw_bones] == names
    raw = [transform(b['local_transform']) for b in raw_bones]
    rows.append(dict(frame=frame['frame'], valid=feet['bValid']['exported_text']=='True',
                     prior=previous, current=current, raw=previous_raw or raw,
                     world=transform(frame['mesh_to_world']), expected=expected))
    previous, previous_raw = current, raw
(out/'native-input.json').write_text(json.dumps(dict(names=names, parents=[b['parent_index'] for b in bones], rows=rows),separators=(',',':')))
print('Prepared 318 native source mesh and socket rows',flush=True)
target = load('484/replay-unit-restored.json.gz')
assert not target['failures']
replacements = []
for i,row in enumerate(target['rows'],1):
    baseline, corrected = [row[key]['rig'] for key in ['baseline','corrected']]
    assert row['query']==i and baseline['valid'] and corrected['valid']
    assert baseline['location']==corrected['location']
    replacements.append(dict(query=i, baseline=baseline, corrected=corrected))
assert len(replacements)==318
assert sum(a!=b for row in replacements for a,b in zip(row['baseline']['rotation'],row['corrected']['rotation']))==103
(out/'vm-input.json').write_text(json.dumps(replacements,separators=(',',':')))
print('Prepared 103 changed RigInput sides from 318 validated queries',flush=True)
