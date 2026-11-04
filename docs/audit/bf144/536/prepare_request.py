import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'534'
request=json.loads((previous/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='depth-473')
request['cases']=[]
for label,offset,observe in [('near',230,True),('edge',237,True),('edge-unobserved',237,False)]:
    case=copy.deepcopy(template)
    case.update(name=label,landing_deck_x_cm=offset,landing_deck_top_cm=18)
    case['frames'][40]['delta_seconds']=.03
    if not observe:
        for frame in (40,41,42):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
(root/'walking-first-cm.json').write_bytes((previous/'walking-first-cm.json').read_bytes())
print('Prepared a real walking step with two ledge landing offsets',flush=True)
