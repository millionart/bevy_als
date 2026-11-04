import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '550' / 'roof.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'clear')
request['cases'] = []
for label, front in (('stalled', 132.25), ('moving', 136)):
    case = copy.deepcopy(template)
    del case['step_roof_bottom_cm']
    case.update(name=label, stall_front_cm=front)
    case['frames'][40]['delta_seconds'] = .06
    request['cases'].append(case)
(root / 'stall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equal 60 ms input with zero and positive first-step displacement', flush=True)
