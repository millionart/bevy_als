import collections
import gzip
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
def read(name):
    path = root / name
    if path.exists():
        return json.loads(path.read_bytes())
    return json.loads(gzip.decompress((root / (name + '.gz')).read_bytes()))

def f32(value):
    return struct.unpack('f', struct.pack('f', value))[0]

def meters(value):
    return dict(value, translation=[x * 0.01 for x in value['translation']])

def projected(value):
    x, y, z, w = value['rotation']
    swap = lambda v: [v[0], v[2], v[1]]
    return meters(dict(translation=swap(value['translation']), rotation=[-x,-z,-y,w], scale=swap(value['scale'])))

def changes(a, b):
    result = {}
    for key in ('translation', 'rotation', 'scale'):
        left, right = a[key], b[key]
        if key == 'rotation' and sum(x*y for x,y in zip(left,right)) < 0:
            right = [-x for x in right]
        if left != right:
            result[key] = [left, right, list(map(f32,left)) != list(map(f32,right))]
    return result

observed = read('replay-unit-baseline.json')
data = read('replay-unit-controls.json')
assert observed['failures'] == data['failures'] == 0
assert len(data['rows']) == 2604
counts = collections.Counter()
source_actual = {}
for old, row in zip(observed['rows'], data['rows'], strict=True):
    controls = row['result'].pop('controls')
    old['result']['public_raw_exact'] = None if old['domain'] == 'source' else old['result']['raw_exact']
    del old['result']['raw_exact']
    assert row == old
    row['result']['controls'] = controls
    counts[row['domain'] + ':' + row['result']['decision']] += 1
    if row['domain'] == 'source' and row['dataset'] == 'actual' and row['pc'] == 130:
        key = row['query']
        if key in source_actual:
            assert source_actual[key] == row['result']
        source_actual[key] = row['result']
assert len(source_actual) == 318
assert counts == {'source:equals':1322, 'source:zero':2, 'source:write':8, 'current:equals':1272}

names = read('inputs.json')['names']
summary = collections.defaultdict(collections.Counter)
by_bone = collections.defaultdict(collections.Counter)
first = {}
ball_inputs = 0
ball_witnesses = {}
def compare(domain, space, row, values_a, values_b):
    assert len(values_a) == len(values_b) == 79
    for i, (a,b) in enumerate(zip(values_a, values_b)):
        bone = names[i]
        for field, (left,right,float_diff) in changes(a,b).items():
            key = f'{domain}:{space}:{field}'
            summary[key]['double'] += 1
            summary[key]['f32'] += float_diff
            summary[key]['max_component_abs'] = max(summary[key]['max_component_abs'], max(abs(x-y) for x,y in zip(left,right)))
            by_bone[key][bone] += float_diff
            if bone.startswith('ball_') and float_diff and (key, bone) not in ball_witnesses:
                ball_witnesses[key, bone] = dict(query=row['query'], dataset=row['dataset'], case=row['case'], mode=row['mode'], left=left, right=right)
            if float_diff and key not in first:
                first[key] = dict(bone=bone, query=row['query'], dataset=row['dataset'], case=row['case'], mode=row['mode'], left=left, right=right)

for row in data['rows']:
    result = row['result']
    if not result['local']:
        continue
    if row['domain'] == 'source':
        cm, m = result['controls']
        assert [cm['unit'], m['unit']] == [1.0, 0.01]
        for side in ['l', 'r']:
            ball = names.index('ball_' + side)
            foot = names.index('foot_' + side)
            assert meters(cm['local'][ball]) == projected(result['local'][ball])
            assert meters(cm['global'][foot]) == projected(result['global'][foot])
            ball_inputs += 1
        for space in ['local','global']:
            native = list(map(projected, result[space]))
            target_cm = list(map(meters, cm[space]))
            compare('same_cm_inputs', space, row, native, target_cm)
            compare('unit_phase_only', space, row, target_cm, m[space])
    else:
        source = source_actual[row['query']]
        for space in ['local','global']:
            compare('actual_histories', space, row, list(map(projected,source[space])),result[space])

assert ball_inputs == 1332
assert summary['unit_phase_only:local:translation']['f32'] == 2654
assert summary['unit_phase_only:global:translation']['f32'] == 0
report = dict(gate='PASS_EXACT_FINAL_SOURCE_AND_PUBLIC_POSES_WITH_SAME_INPUT_ARITHMETIC_CONTROLS',
              source_frames=666, current_frames=636, source_setters=1332, current_setters=1272,
              bones_per_pose=79, held_ball_local_and_foot_global_inputs=ball_inputs, setter_decisions=counts, comparisons=summary,
              ball_witnesses={key+':'+bone: value for (key,bone),value in ball_witnesses.items()},
              f32_differences_by_bone=by_bone, first_f32_differences=first,
              equal_rotations_compare_up_to_quaternion_sign=True)
(root/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(dict(setter_decisions=counts, comparisons=summary),sort_keys=True),flush=True)
