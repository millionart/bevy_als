import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
request=json.loads((root.parent/'520'/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='unblocked')
request['cases']=[]
for label,width,observe in [('width-100',100,True),('width-108',108,True),('width-125',125,True),('width-100-unobserved',100,False)]:
    case=copy.deepcopy(template)
    case.update(name=label,floor_half_extent_x_cm=width)
    case['frames']=case['frames'][:43]
    case['frames'][40].update(delta_seconds=.06,walking_initial_cm=[99,0,92.15,600,0,0])
    if observe:
        for ordinal in (40,41,42):
            case['frames'][ordinal]['observe_blocking_hits']=True
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared stable Walking start and three platform widths',flush=True)
