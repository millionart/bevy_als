import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '566' / 'bases.request.json').read_bytes())
template = request['cases'][1]
request['cases'] = []
initial = json.loads((root / 'initial.json').read_bytes())
for name, simulated, enabled in (('dynamic', True, True), ('disabled', True, False), ('static', False, True)):
    case = copy.deepcopy(template)
    del case['walk_off_step_moves']
    case.update(name=name, landing_deck_top_cm=-50, landing_deck_x_cm=230, simulate_landing_deck=simulated, enable_impact=enabled)
    case['frames'] = case['frames'][:43]
    frame = case['frames'][40]
    del frame['walking_initial_cm']
    frame.update(delta_seconds=initial['delta_s'], falling_initial_cm=[230, -30, 57, 0, 0, -500],
                 movement_input_world=dict(x=0, y=0, z=0))
    request['cases'].append(case)
(root / 'impact.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared simulated landing, disabled interaction, and static deck controls', flush=True)
