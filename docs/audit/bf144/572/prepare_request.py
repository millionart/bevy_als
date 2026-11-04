import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '571' / 'full-input.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'single')
request['capture_pose_boundaries'] = True
request['capture_rig_output'] = True
request['cases'] = []
for name, delta, stop in (('coarse', .125, 62), ('fixed', .03125, 92)):
    case = copy.deepcopy(template)
    case['name'] = name
    for frame in range(52, 105):
        record = copy.deepcopy(case['frames'][51])
        record['delta_seconds'] = delta
        record['desired_gait'] = 'Als.Gait.Sprinting' if frame < stop else 'Als.Gait.Running'
        record['movement_input_world']['x'] = 1 if frame < stop else 0
        case['frames'].append(record)
    request['cases'].append(case)
(root / 'pelvis.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equivalent-duration source run-off and landing schedules', flush=True)
