import copy
import json
from pathlib import Path

root=Path(__file__).resolve().parent
request=json.loads((root.parent/'537'/'wall.request.json').read_bytes())
template=next(case for case in request['cases'] if case['name']=='supported')
request['cases']=[]
for label,observe in [('grazing',True),('grazing-unobserved',False)]:
    case=copy.deepcopy(template)
    case['name']=label
    if not observe:
        for frame in (40,41,42):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared the native grazing step and observation-off control',flush=True)
