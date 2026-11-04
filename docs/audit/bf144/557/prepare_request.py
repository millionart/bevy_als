import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '555' / 'apex.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'clear')
request['cases'] = []
for label, axis in (('toward', 1), ('none', 0)):
    case = copy.deepcopy(template)
    del case['force_apex_split']
    del case['apex_roof']
    case.update(name=label)
    case['frames'][49].update(falling_initial_cm=[200, 0, 200, 400, 200, -200],
                              movement_input_world={'x': 0, 'y': axis, 'z': 0})
    request['cases'].append(case)
(root / 'corner.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared no-input and into-second-slope native Air requests', flush=True)
