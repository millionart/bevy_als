import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '572' / 'pelvis.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'coarse')
del request['capture_pose_boundaries'], request['capture_rig_output']
request['cases'] = []
for name, direction in (('axial', 0), ('positive', 1), ('negative', -1)):
    case = copy.deepcopy(template)
    case['name'] = name
    case['frames'] = case['frames'][:60]
    if direction:
        case['frames'][54]['movement_input_world'] = dict(x=1-2**-21, y=direction * 2**-10, z=0)
    request['cases'].append(case)
(root / 'lateral.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared exact axial and signed physical-direction controls', flush=True)
