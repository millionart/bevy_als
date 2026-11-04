import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
request=json.loads((root/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='depth-475')
request['cases']=[]
for label,observe in [('depth-47403',True),('depth-47403-unobserved',False)]:
    case=copy.deepcopy(template)
    case.update(name=label,landing_deck_top_cm=-47.403)
    if not observe:
        for frame in (40,41,42):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'boundary.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared one shared deck depth between the measured floor thresholds',flush=True)
