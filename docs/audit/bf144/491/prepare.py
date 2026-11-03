import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent/'457/request.json').read_text())
request['cases'][0]['frames'] = request['cases'][0]['frames'][:8]
def save(label, value):
    (root/f'{label}.request.json').write_text(json.dumps(value,indent=2)+'\n')

save('legacy',request)
control = copy.deepcopy(request)
template = control['cases'][0]
for initial in [True,False]:
    case = copy.deepcopy(template)
    case['name'] = 'initial-ik' if initial else 'initial-virtual'
    case['initial_use_foot_ik_bones'] = initial
    case['frames'][2]['use_foot_ik_bones'] = not initial
    case['frames'][5]['use_foot_ik_bones'] = initial
    control['cases'].append(case)
control['cases'].pop(0)
save('switch',control)
invalid = copy.deepcopy(request)
invalid['cases'][0]['initial_use_foot_ik_bones'] = 'false'
save('invalid',invalid)
print('Prepared two independent initial settings, switch controls and a strict type negative',flush=True)
