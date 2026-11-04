import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
request=json.loads((root.parent/'537'/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='miss')
request['cases']=[]
for label,yaw,height,observe in (
    ('allow',0,88,True),('opposed',15,88,True),('high',0,94,True),('allow-unobserved',0,88,False)
):
    case=copy.deepcopy(template)
    case.update(name=label,step_slope_yaw=yaw,step_slope_contact_height_cm=height)
    if not observe:
        for frame in (40,41,42):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared low allowed, low opposed and high StepDown slopes',flush=True)
