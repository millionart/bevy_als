import ast
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
def load(name):
    return json.loads((root/name).read_bytes())

def props(rows):
    return {row['name']:row for row in rows}

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

for label in ('movement','profiles'):
    assert load(f'{label}.native.json.reader.report.json')['success']
movement = load('movement.native.json')
modes = props(movement['asset_properties'])['RotationModes']['elements']
velocity = next(row for row in modes if 'VelocityDirection' in row['key'])
stances = props(velocity['fields'])['Stances']['elements']
stance_fields = {row['key'].split('"')[1].split('.')[-1]:props(row['fields']) for row in stances}
def reference(stance,key):
    return stance_fields[stance][key]['exported_text'].split("'")[1]
curve_key = 'AccelerationAndDecelerationAndGroundFrictionCurve'
normal = reference('Standing',curve_key)
responsive = reference('Crouching',curve_key)
rotation = reference('Standing','RotationInterpolationSpeedCurve')
assert normal.endswith('_Normal.CV_Als_AccelerationAndDecelerationAndGroundFriction_Normal')
assert responsive.endswith('_Responsive.CV_Als_AccelerationAndDecelerationAndGroundFriction_Responsive')
assert rotation.endswith('_Responsive.CF_Als_RotationInterpolationSpeed_Responsive')
target = load('runtime-baseline/profile.json')['original']
samples = load('profiles.native.json')['blend_profile_probe']['samples']
assert len(samples)==6
fit = ast.parse(Path('tools/export/fit_ground_dynamics_curves.py').read_text())
functions = [node for node in fit.body if isinstance(node,ast.FunctionDef) and node.name in ('basis','solve_tangents')]
assert len(functions)==2
scope = {}
exec(compile(ast.Module(body=functions,type_ignores=[]),'project Hermite fitter','exec'),scope)
channel_names = ('acceleration','braking_deceleration','ground_friction')
starts = {key:[None,None,None] for key in ('normal','responsive')}
corrected = []
for sample in samples:
    profile = {normal:'normal',responsive:'responsive'}[sample['custom_curve']]
    axis = sample['custom_curve_axis']
    channel = channel_names[axis]
    raw_keys = sample['custom_curve_keys']
    assert len(raw_keys)==4 and len(sample['custom_curve_time_samples'])==61
    scale = .01 if axis < 2 else 1
    current = target[profile][channel]['keys']
    assert [row['time'] for row in current] == [row['time'] for row in raw_keys]
    assert [row['value'] for row in current] == [f32(row['value']*scale) for row in raw_keys]
    starts[profile][axis] = current[0]['value']
    if (profile,axis) in (('normal',0),('responsive',1)):
        raw_tangent = f32(raw_keys[2]['leave_tangent']*scale)
        observed = current[2]['leave_tangent']
        values = {row['time']:row['value'] for row in sample['custom_curve_time_samples']}
        fitted = f32(scope['solve_tangents'](*(values[t] for t in (2,3,2.25,2.75)))[0]*scale)
        assert observed == fitted != raw_tangent
        corrected.append(dict(profile=profile,channel=channel,key=2,current=observed,source=raw_tangent,fitted=fitted))
assert starts==dict(normal=[20,15,5],responsive=[25,20,10])
assert len(corrected)==2
(root/'profile-facts.json').write_text(json.dumps(dict(source_movement_asset=movement['asset_path'],
    normal=normal,responsive=responsive,rotation=rotation,starts=starts,tangent_controls=corrected),
    indent=2)+'\n')
print('Verified six original channels, both first samples and two D267 tangent fits',flush=True)
