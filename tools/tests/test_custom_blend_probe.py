import json
import sys
import struct
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from ue_exported_text import parse_list, parse_struct

source = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
curve = next(p for p in source['asset_properties'] if p['name'] == 'FloatCurve')
fields = {p['name']: p['exported_text'] for p in curve['fields']}
assert fields['PreInfinityExtrap'] == fields['PostInfinityExtrap'] == 'RCCE_Constant'
keys = [parse_struct(value) for value in parse_list(fields['Keys'])]
value = lambda key, name: float(key.get(name, '0'))

def sample(time):
    if time <= value(keys[0], 'Time'):
        return value(keys[0], 'Value')
    if time >= value(keys[-1], 'Time'):
        return value(keys[-1], 'Value')
    a, b = next((a, b) for a, b in zip(keys, keys[1:]) if value(a, 'Time') <= time < value(b, 'Time'))
    assert a['InterpMode'] == 'RCIM_Cubic'
    length = value(b, 'Time') - value(a, 'Time')
    t = (time - value(a, 'Time')) / length
    return (2*t**3 - 3*t*t + 1)*value(a, 'Value') + (t**3 - 2*t*t + t)*length*value(a, 'LeaveTangent') + (t**3 - t*t)*length*value(b, 'ArriveTangent') + (-2*t**3 + 3*t*t)*value(b, 'Value')

request = json.loads(Path(__file__).with_name('fixtures').joinpath('custom_blend.request.json').read_text())['samples']
rows = json.loads(Path(sys.argv[2]).read_text(encoding='utf-8-sig'))['blend_profile_probe']['samples']
assert len(rows) == len(request) == 1
for expected, row in zip(request, rows):
    assert row['name'] == expected['name'] and row['custom_curve'] == source['asset_path']
    assert len(row['frames']) == len(expected['deltas'])
    assert len(row['custom_curve_keys']) == len(keys)
    for key in row['custom_curve_keys']:
        assert set(key) == {'time', 'value', 'arrive_tangent', 'leave_tangent'}
        assert all(struct.unpack('f', struct.pack('f', number))[0] == number for number in key.values())
    for frame in row['frames']:
        time = value(keys[0], 'Time') + (value(keys[-1], 'Time') - value(keys[0], 'Time')) * frame['factor']
        assert abs(frame['alpha'] - min(max(sample(time), 0), 1)) < 3e-6
        assert abs(frame['blend_value'] - frame['alpha']) < 3e-6
    assert rows[0]['frames'][0]['alpha'] == 0
    assert rows[0]['frames'][-1]['alpha'] == 1 and rows[0]['frames'][-1]['complete']
print('Native custom blend curve sampling and clock PASS')
