import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '547' / 'edge.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'upper')
request['cases'] = []
for label, front in (('zero', 138.45), ('micro', 138.638), ('advance', 138.7)):
    case = copy.deepcopy(template)
    del case['step_ceiling_bottom_cm']
    case.update(name=label, step_stop_front_cm=front)
    request['cases'].append(case)
(root / 'stop.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared zero, sub-epsilon and above-epsilon axial StepUp cases', flush=True)
