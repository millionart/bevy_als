import json
import struct
import sys
from pathlib import Path

f32=lambda value:struct.unpack('f',struct.pack('f',value))[0]
request=json.loads(Path(__file__).with_name('fixtures').joinpath('blend_list_parent_weight.request.json').read_text())
probe=json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['blend_list_probe']
assert len(probe['samples'])==len(request['samples'])==4
for case,row in zip(request['samples'],probe['samples']):
    assert case['name']==row['name'] and len(row['frames'])==1
    step=case['steps'][0];frame=row['frames'][0]
    parent=f32(step.get('parent_weight',1));alpha=f32(step['outer_alpha'])
    expected=max(f32(parent*f32(1-alpha)),f32(parent*alpha))
    assert frame['cached_updates']==[expected],(case['name'],frame['cached_updates'],expected)
print('Native fractional parent weight and cached maximum update weight PASS: 4 frames',flush=True)
