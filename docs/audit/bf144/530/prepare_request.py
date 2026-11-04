import copy
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '528'
request = json.loads((prior / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'top-205')
initial = json.loads((root.parent / '523' / 'initial.json').read_bytes())
sphere = list(initial['position_m'])
sphere[1] -= .6
normal = [.5, math.sqrt(.5), .5]
distance = .3 + .002 * normal[1] + .015
center = [value - axis*distance for value,axis in zip(sphere,normal,strict=True)]
fixture = dict(blocker_center_m=center, blocker_size_m=[.14,.03,1.6],
               deck_x_m=2.24, deck_top_m=1.978)
(root/'fixture-input.json').write_text(json.dumps(fixture,indent=2)+'\n')
relative = [(center[0]+15)*100,(center[2]+15)*100,(center[1]-2)*100]
request['cases'] = []
for label, blocker, observe in [('line',True,True),('no-blocker',False,True),('line-unobserved',True,False)]:
    case = copy.deepcopy(template)
    case.update(name=label,landing_deck_top_cm=-2.2)
    if blocker:
        case['floor_blocker_cm']=relative
    for frame in (49,50,51):
        if observe:
            case['frames'][frame]['observe_blocking_hits']=True
        else:
            case['frames'][frame].pop('observe_blocking_hits',None)
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
point=json.loads((prior/'summary.json').read_bytes())['native']['top-205']['first_hit']['move_stop_cm']
mapped=[point['x']/100,2+point['z']/100,point['y']/100]
(root/'native-origins.json').write_text(json.dumps({label:mapped for label in ('line','no-blocker')},indent=2)+'\n')
print('Prepared slanted floor blocker, unobstructed center ray and no-blocker control',flush=True)
