import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '527' / 'wall.request.json').read_bytes())
cases = {case['name']: case for case in request['cases']}
request['cases'] = []
for label, source, deck, observe in (
    ('top-205','top-205',-2.7,True),
    ('top-215','top-215',-2.7,True),
    ('top-225','top-225',-2.7,True),
    ('top-215-raised','top-215',-2.2,True),
    ('top-215-raised-unobserved','top-215',-2.2,False),
):
    case = copy.deepcopy(cases[source])
    case.update(name=label, landing_deck_top_cm=deck)
    if not observe:
        for frame in (49,50):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root / 'wall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared same wall and raised deck controls for the secondary floor hit', flush=True)
