import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '555' / 'apex.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'clear')
request['cases'] = []
for label, crouch in (('crouched', True), ('standing', False)):
    case = copy.deepcopy(template)
    del case['force_apex_split']
    del case['apex_roof']
    case.update(name=label)
    for i, frame in enumerate(case['frames']):
        frame['desired_stance'] = 'Als.Stance.Crouching' if crouch else 'Als.Stance.Standing'
        frame['desired_gait'] = 'Als.Gait.Running'
        frame['movement_input_world'] = {'x': int(49 <= i <= 51), 'y': 0, 'z': 0}
    case['frames'][49]['falling_initial_cm'] = [200, 0, 200, 200, 0, 0]
    request['cases'].append(case)
(root / 'speed.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equal airborne requests after crouched and standing Walking', flush=True)
