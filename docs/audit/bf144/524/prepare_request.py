import copy
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '523'
read = lambda path: json.loads(path.read_bytes())
request = read(prior / 'wall.request.json')
template = next(case for case in request['cases'] if case['name'] == 'wall-deck')
geometry = read(root.parent / '521' / 'geometry.request.json')
sample = next(row for row in geometry['samples'] if row['name'] == 'falling-0')
request['cases'] = []
geometry['samples'] = []
for name, pitch in [('positive', -.003), ('negative', .003)]:
    case = copy.deepcopy(template)
    case.update(name=name, wall_pitch_degrees=pitch)
    request['cases'].append(case)
    row = copy.deepcopy(sample)
    row['name'] = name
    half_pitch = math.radians(-pitch) / 2
    half_yaw = math.pi / 8
    cp, sp = math.cos(half_pitch), math.sin(half_pitch)
    cy, sy = math.cos(half_yaw), math.sin(half_yaw)
    row['rotation_xyzw'] = [-sy * sp, cy * sp, sy * cp, cy * cp]
    geometry['samples'].append(row)
(root / 'wall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
(root / 'geometry.request.json').write_text(json.dumps(geometry, indent=2) + '\n')
print('Prepared common positive and negative wall pitches and native sweep inputs', flush=True)
