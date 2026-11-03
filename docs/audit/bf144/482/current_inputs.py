import gzip
import json
import math
import re
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def read(path):
    p=Path(path)
    if not p.exists():
        p=Path(str(p)+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
part=r'RigTransform \{ rotation: DQuat\(([^)]*)\), translation: DVec3\(([^)]*)\), scale: DVec3\(([^)]*)\) \}'
pattern=re.compile(r'BonePose \{ local: '+part+r', global: '+part+r', dirty: (None|Some\((?:Local|Global)\)) \}')
def parse(value):
    matches=list(pattern.finditer(value))
    assert len(matches)==79
    assert pattern.sub('',value).replace(', ','')==''
    rows=[]
    for match in matches:
        values=match.groups()
        def transform(offset):
            arrays=[[float(v)for v in values[offset+i].split(', ')]for i in range(3)]
            assert list(map(len,arrays))==[4,3,3]and all(math.isfinite(v)for a in arrays for v in a)
            return dict(rotation=arrays[0],translation=arrays[1],scale=arrays[2])
        dirty=values[6]
        rows.append(dict(local=transform(0),global_cache=transform(3),dirty=None if dirty=='None'else dirty[5:-1]))
    return rows


data = read(r/'replay-baseline.json')
print('Loaded observed VM', flush=True)
print('Parsing setter cache records', flush=True)
indices = {bone['name']: i for i, bone in enumerate(read('docs/audit/bf144/476/inputs.json')['definition']['bones'])}
current = []
for mode, stream in enumerate(data['modes']):
    for frame in stream['outputs']:
        if frame['query'] % 32 == 0:
            print(f'Parsing mode {mode} frame {frame["query"]}', flush=True)
        globals_ = frame.pop('final_globals')
        for call in frame['calls']:
            before = call.pop('setter_raw_before')
            after = call.pop('setter_raw_after')
            if before is None:
                assert after is None
                continue
            args = call['arguments_before_call']
            assert args[1:3] == [{'Name':'GlobalSpace'}, {'Bool':False}]
            assert args[5] == {'Bool':True}
            foot = indices[args[0]['List'][1]['Name']]
            entry = dict(mode=mode, query=frame['query'], pc=call['instruction'],
                         foot=foot, before=parse(before), expected=parse(after),
                         weight=args[4]['F32'], rotation=args[3]['Quaternion'],
                         expected_foot=call['bone_after_call'])
            if call['instruction'] == 130:
                entry.update(local=[bone['local'] for bone in frame['local']], globals=globals_)
            current.append(entry)
print('Comparing old VM histories', flush=True)
assert data == read('docs/audit/bf144/481/replay-clean-control.json')
del data
inputs = read(r/'inputs.json')
assert len(current) == 1272 and sum('local' in row for row in current) == 636
inputs['current'] = current
(r/'inputs.json').write_text(json.dumps(inputs, separators=(',', ':'))+'\n')
print('Bound 1272 public setter caches and 636 final poses while preserving the complete old VM histories', flush=True)
