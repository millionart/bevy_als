import json
from pathlib import Path

r = Path(__file__).resolve().parent
data = json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
samples = []
mapping = []
source = []
current = []

def vector(value):
    return [value[k] for k in 'xyz']

def sample(domain, row, bone, a, b, scale):
    index = len(samples)
    samples.append(dict(rotation_xyzw=[0, 0, 0, 1], scaled_distance=dict(a=a, b=b, scale=scale)))
    mapping.append(dict(domain=domain, row=row, bone=bone))
    return index

for row_index, row in enumerate(data['source']):
    args = row['arguments_after_native_call']
    assert args[14:16] == [0, 0]
    assert args[9] == dict(Type='Bone', Name='None')
    assert row['automatic_lengths_requested']
    if row['weight'] <= 1e-8:
        assert not row['changed_by_ik']
        continue
    lengths = []
    for bone in range(2):
        initial = row['initial'][bone]
        initial_scale = vector(initial['scale'])
        assert sum(v * v for v in initial_scale) > 1e-8
        scale = [a / b for a, b in zip(vector(row['before'][0]['scale']), initial_scale)]
        lengths.append(sample('source_cm', row_index, bone, vector(initial['translation']), vector(row['initial'][bone + 1]['translation']), scale))
    source.append(dict(row=row_index, lengths=lengths))

bones = data['definition']['bones']
for row_index, row in enumerate(data['current']):
    args = row['arguments_before']
    assert args[14:16] == [dict(F32=0.0), dict(F32=0.0)]
    assert args[13] == dict(F32=1.0)
    indices = [item[0] for item in row['bones_before']]
    assert [bones[i]['name'] for i in indices] == [args[i]['List'][1]['Name'] for i in range(3)]
    domains = dict(current_m=[], current_cm=[])
    for bone in range(2):
        initial = bones[indices[bone]]['initial_global']
        initial_scale = initial['scale']
        assert sum(v * v for v in initial_scale) > 1e-8
        scale = [a / b for a, b in zip(row['bones_before'][0][1]['scale'], initial_scale)]
        a = initial['translation']
        b = bones[indices[bone + 1]]['initial_global']['translation']
        domains['current_m'].append(sample('current_m', row_index, bone, a, b, scale))
        lift = lambda v: [v[0] * 100.0, v[2] * 100.0, v[1] * 100.0]
        domains['current_cm'].append(sample('current_cm', row_index, bone, lift(a), lift(b), [scale[0], scale[2], scale[1]]))
    current.append(dict(row=row_index, lengths=domains))

assert len(source) == 1330 and len(current) == 1272 and len(samples) == 7748
(r / 'lengths.request.json').write_text(json.dumps(dict(samples=samples), separators=(',', ':')) + '\n')
(r / 'mapping.json').write_text(json.dumps(dict(source=source, current=current, samples=mapping), separators=(',', ':')) + '\n')
print('Prepared2660 source and5088 same-current native length calculations; two zero-weight source calls return before lengths', flush=True)
