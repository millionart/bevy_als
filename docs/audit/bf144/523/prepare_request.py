import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '521' / 'runtime-wall-162-aligned-air'
trace = json.loads((previous / 'wall-trace.json').read_bytes())
fixture = json.loads((previous / 'fixture.json').read_bytes())
comparison = json.loads((root.parent / '520' / 'actual-comparison.json').read_bytes())
alignment = json.loads((previous / 'alignment.json').read_bytes())
start = trace['departure']
velocity = comparison['target']['wall-162-aligned-air']['walking_input']
origin = [fixture['origin'][0], fixture['floor_y'], fixture['origin'][2]]
initial = dict(position_m=start, velocity_mps=velocity, delta_s=trace['air_time'],
               air_maximum_mps2=alignment['air_maximum'], deck_top_m=1.973)
(root / 'initial.json').write_text(json.dumps(initial, indent=2) + '\n')
def ue(vector):
    return [vector[index] * 100 for index in (0, 2, 1)]
state = ue([a-b for a,b in zip(start, origin, strict=True)]) + ue(velocity)
request = json.loads((root.parent / '520' / 'wall.request.json').read_bytes())
prior = next(row for row in request['cases'] if row['name'] == 'wall-162')
request['cases'] = []
for name, deck in [('wall-deck', True), ('wall-only', False)]:
    case = copy.deepcopy(prior)
    case['name'] = name
    case['frames'] = case['frames'][:52]
    case['frames'][49].update(delta_seconds=initial['delta_s'], falling_initial_cm=state)
    if deck:
        case['landing_deck_top_cm'] = (initial['deck_top_m'] - origin[1]) * 100
    request['cases'].append(case)
(root / 'wall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared identical initial world state and wall-deck geometry', flush=True)
