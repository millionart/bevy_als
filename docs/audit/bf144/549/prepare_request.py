import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '548' / 'stop.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'zero')
request['cases'] = []
for label, height, center in (('lower', 44.6, 92.15), ('near-limit', 44.9, 92.15), ('tight-limit', 44.9, 90.3)):
    case = copy.deepcopy(template)
    del case['step_fin']
    del case['step_stop_front_cm']
    case.update(name=label, step_limit_height_cm=height)
    case['frames'][40]['walking_initial_cm'][2] = center
    request['cases'].append(case)
(root / 'height.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared two sub-limit step heights bracketing the target center-height guard', flush=True)
