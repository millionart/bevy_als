import json
import sys
from pathlib import Path

data=json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['blend_list_probe']['samples']
frames=data[0]['frames']
assert len(data)==1 and len(frames)==34
assert frames[0]['weights']==[1,0] and frames[0]['duration']==0
assert all(row['initializations']==[1,1] for row in frames)
assert frames[4]['weights']==[0.9803240895271301,0.019675929099321365]
assert frames[9]['weights']==[.5,.5]
assert abs(frames[10]['duration']-.15)<1e-7
assert frames[-1]['weights']==[1,0]
for row in frames:
    assert abs(sum(row['weights'])-1)<1e-6
    for weight,calls in zip(row['weights'],row['updates']):
        assert calls==([weight] if weight>1e-5 else [])
print('Native blend-list initialization, interrupted duration and child update weights PASS')
