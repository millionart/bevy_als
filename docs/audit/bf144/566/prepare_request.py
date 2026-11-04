import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '551' / 'stall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'moving')
request['cases'] = []
for label, moving, dt in (('moving', True, .1), ('still', False, .1), ('first-step', True, .05)):
    case = copy.deepcopy(template)
    del case['stall_front_cm']
    case.update(name=label, floor_half_extent_x_cm=100, walk_off_step_moves=moving)
    case['frames'][40]['delta_seconds'] = dt
    request['cases'].append(case)
(root / 'bases.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equal first-step and complete two-step support cases', flush=True)
