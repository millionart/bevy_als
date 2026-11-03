import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/494')
temporary.mkdir(parents=True,exist_ok=True)
def read(path):
    with gzip.open(parent/path,'rt') as stream:
        return json.load(stream)

def transform(value):
    return {field:[value[field][axis] for axis in axes]
            for field,axes in [('translation','xyz'),('rotation','xyzw'),('scale','xyz')]}

case = read('493/virtual-motion.native.json.gz')['character_animation_probe']['cases'][0]
markers = ['root','VB foot_root','VB foot_l','VB foot_r']
def pose(frame):
    by_name = {bone['name']:bone['local'] for bone in frame['mesh_local_pose']}
    return [transform(by_name[name]) for name in markers]

def named(values,name):
    return next(value for value in values if value['name']==name)

previous = pose(case['initialization'])
rows = []
parents = None
for frame in case['frames']:
    full = frame['pose_boundaries']['root_result']['pose']['bones']
    indices = {value['name']:i for i,value in enumerate(full)}
    parent_indices = [full[indices[name]]['parent_index'] for name in markers]
    current_parents = [-1 if i<0 else markers.index(full[i]['name']) for i in parent_indices]
    if parents is not None:
        assert current_parents==parents
    parents = current_parents
    feet = named(frame['source_state_after_tick'],'FeetState')['fields']
    expected = []
    for side in ['Left','Right']:
        fields = named(feet,side)['fields']
        expected.append([[part['numeric_value'] for part in named(fields,f'Target{kind}WorldSpace')['fields']]
                         for kind in ['Location','Rotation']])
    rows.append(dict(frame=frame['frame'],valid=named(feet,'bValid')['exported_text']=='True',
                     prior=previous,world=transform(frame['mesh_to_world']),expected=expected))
    previous = pose(frame)
assert len(rows)==158 and parents==[-1,0,1,1]
(temporary/'source.json').write_text(json.dumps(dict(names=markers,parents=parents,rows=rows),separators=(',',':')))
print('Prepared 157 native valid virtual-foot socket pairs',flush=True)

before = read('493/runtime-virtual/parameters.json.gz')['animation_before']
target = [dict(query=i,observation=row['foot_feedback'],prior=before[i-1]['foot_feedback'])
          for i,row in enumerate(before) if i>0]
assert len(target)==158 and all(row['observation']['valid'] and not row['observation']['use_ik'] for row in target)
(temporary/'target.json').write_text(json.dumps(dict(rows=target),separators=(',',':')))
print('Prepared 158 real virtual-foot target observations and their prior state',flush=True)

scale = (parent/'488/replay.rs').read_text()
for old,new in [('bf144/488','bf144/494'),('../486/replay.rs','../../../../docs/audit/bf144/486/replay.rs'),
                ('ik_foot_root','VB foot_root'),('ik_foot_l','VB foot_l'),('ik_foot_r','VB foot_r'),
                ('(634, 636)','(314, 316)'),('634 actual native sockets and 636 actual','314 actual native sockets and 316 actual')]:
    assert old in scale
    scale=scale.replace(old,new)
scale=scale.replace('fn main() {','pub fn run(label: &str) -> Value {')
scale=scale.replace('    let label = std::env::args().nth(1).unwrap();\n','')
scale=scale.replace('use bevy_als::extensions::control_rig_hierarchy::RigTransform;\n','',1)
scale=scale[:scale.rfind('}')]+ '    report\n}\n'
(temporary/'scale.rs').write_text(scale)

previous = parent/'487/prepare.py'
fixture = previous.read_text()
fixture=fixture[fixture.index('production = '):fixture.index('\nvm = ')]
exec(compile(fixture,previous.name,'exec'))
production = temporary/'production.rs'
production.write_text(production.read_text().replace('crate::als::foot_ik::','bevy_als::als::foot_ik::'))
fixture=(parent/'487/replay.rs').read_text()
a=fixture.index('mod als {')
b=fixture.index('fn read(',a)
c=fixture.index('fn project(',b)
d=fixture.index('fn foot_case(',c)
e=fixture.index('fn main()',d)
includes=fixture[fixture.index('include!(',a):fixture.index('mod vm {',a)]
fixture=(fixture[:a]+includes+fixture[b:c]+fixture[d:e]).replace('/487/','/494/')
fixture=fixture.replace('fn foot_case(','pub fn foot_case(')
(temporary/'foot.rs').write_text(fixture)
print('Reused calibrated scale and fixed-history foot consumer source',flush=True)
