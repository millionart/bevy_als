import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
request=json.loads((root.parent/'536'/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='near')
request['cases']=[]
for label,width,observe in [('miss',100,True),('supported',145,True),('miss-unobserved',100,False)]:
    case=copy.deepcopy(template)
    case.pop('landing_deck_x_cm')
    case.pop('landing_deck_top_cm')
    case.update(name=label,floor_half_extent_x_cm=width,step_fin=True)
    case['frames'][40].update(delta_seconds=.05,walking_initial_cm=[102,0,92.15,650,0,0])
    if not observe:
        for frame in (40,41,42):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared the off-axis step fin and supported floor control',flush=True)
