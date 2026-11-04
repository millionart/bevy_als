import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '549' / 'height.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'lower')
request['cases'] = []
for label, bottom in (('limited', 225.6), ('clear', 228)):
    case = copy.deepcopy(template)
    del case['step_limit_height_cm']
    case.update(name=label, step_roof_bottom_cm=bottom)
    request['cases'].append(case)
(root / 'roof.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equal 44 cm steps with a limited and clear post-step adjustment', flush=True)
