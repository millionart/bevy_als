import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent/'580'/'ditch.request.json').read_bytes())
template = request['cases'][0]
request['cases'] = []
for label, angle in (('near-normal', .5), ('outside-angle', 1.25), ('single', 0)):
    case = copy.deepcopy(template)
    del case['virtual_ditch_wall_gap_cm']
    case.update(name=label, repeated_wall_yaw=angle)
    case['frames'][49]['falling_initial_cm'] = [200,0,200,200,400,-200]
    request['cases'].append(case)
(root/'nudge.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared two signed normal-angle controls and one-wall control',flush=True)
