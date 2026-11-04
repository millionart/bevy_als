import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'534'
request=json.loads((previous/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='depth-473')
request['cases']=[]
for label,depth,observe in [('clamp-off',47.0,True),('clamp-on',47.35,True),('clamp-on-unobserved',47.35,False)]:
    case=copy.deepcopy(template)
    case.update(name=label,landing_deck_x_cm=200.5,landing_deck_top_cm=-depth)
    case['frames'][40]['walking_initial_cm']=[101,0,27.1,0,0,0]
    for frame in (40,41,42):
        case['frames'][frame]['movement_input_world']={'x':0,'y':0,'z':0}
        if not observe:
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
(root/'walking-first-cm.json').write_text(json.dumps(dict(x=-1399.0,y=-1500.0,z=27.1),indent=2)+'\n')
print('Prepared a uniformly scaled capsule and two valid perch distances',flush=True)
