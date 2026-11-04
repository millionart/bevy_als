import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'526'
request=json.loads((prior/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='edge-in')
request['cases']=[]
positions={'near':230,'mid':248,'far':264}
for label,x,observe in [(name,x,True) for name,x in positions.items()]+[('mid-unobserved',248,False)]:
    case=copy.deepcopy(template)
    case.update(name=label,landing_deck_x_cm=x)
    for frame in (49,50,51):
        if observe:
            case['frames'][frame]['observe_blocking_hits']=True
        else:
            case['frames'][frame].pop('observe_blocking_hits',None)
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
point=json.loads((prior/'summary.json').read_bytes())['native']['edge-in'][0]['hits'][0]['move_stop_cm']
mapped=[point['x']/100,2+point['z']/100,point['y']/100]
(root/'native-origins.json').write_text(json.dumps({name:mapped for name in positions},indent=2)+'\n')
print('Prepared the same walkable wall top with three perch deck offsets',flush=True)
