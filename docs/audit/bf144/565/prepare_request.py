import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '564' / 'speed.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'standing')
request['cases'] = []
for label, switch in (('switch', True), ('same', False)):
    case = copy.deepcopy(template)
    case.update(name=label, jump_base_switch=switch)
    for frame in case['frames']:
        frame['movement_input_world'] = {'x': 0, 'y': 0, 'z': 0}
        frame.pop('falling_initial_cm', None)
    request['cases'].append(case)
(root / 'jump.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared old-base and same-frame new sensor support cases', flush=True)
