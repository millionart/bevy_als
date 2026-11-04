import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'533'
request=json.loads((prior/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='width-100')
request['cases']=[]
for label,depth,observe in [('depth-473',47.3,True),('depth-475',47.5,True),('depth-475-unobserved',47.5,False)]:
    case=copy.deepcopy(template)
    case.update(name=label,landing_deck_x_cm=205,landing_deck_top_cm=-depth)
    if not observe:
        for frame in (40,41,42):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
source=json.loads((prior/'summary.json').read_bytes())
first=source['native']['width-100']['switch']['location_cm']
(root/'walking-first-cm.json').write_text(json.dumps(first,indent=2)+'\n')
print('Prepared two low deck depths bracketing the walking perch budget',flush=True)
