import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '562' / 'side.request.json').read_bytes())
template = request['cases'][0]
request['cases'] = []
for label, gap in (('tight', .10), ('wide', 1.40), ('without-wall', 0)):
    case = copy.deepcopy(template)
    del case['corner_slope_gap_cm']
    case.update(name=label, virtual_ditch_wall_gap_cm=gap)
    case['frames'] = case['frames'][:51]
    case['frames'][49].update(delta_seconds=.05, falling_initial_cm=[200,0,200,-50,2,-200])
    request['cases'].append(case)
(root / 'ditch.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared a narrow sloped ledge and lateral blocking controls',flush=True)
