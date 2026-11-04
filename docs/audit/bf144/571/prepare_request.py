import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '570' / 'impact.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'static')
request['cases'] = []
for label in ('double', 'single'):
    case = copy.deepcopy(template)
    case['name'] = label
    landing = case['frames'][40]
    case['frames'] = case['frames'][:41]
    for frame in range(41, 76):
        next_frame = copy.deepcopy(landing)
        if label != 'double' or frame != 57:
            del next_frame['falling_initial_cm']
        else:
            next_frame['falling_initial_cm'][2] = 55
        next_frame['delta_seconds'] = .03125
        next_frame['movement_input_world']['x'] = .05 if frame == 56 else 0
        case['frames'].append(next_frame)
    request['cases'].append(case)
(root / 'timer.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared first landing and a second landing on the prior expiry frame', flush=True)
