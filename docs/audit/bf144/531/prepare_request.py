import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'530'
request = json.loads((previous/'wall.request.json').read_bytes())
names = {'line':'perch','line-unobserved':'perch-unobserved','no-blocker':'no-blocker'}
for case in request['cases']:
    case['name'] = names[case['name']]
(root/'wall.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
fixture=json.loads((previous/'fixture-input.json').read_bytes())
fixture['deck_x_m']=2.39
(root/'fixture-input.json').write_text(json.dumps(fixture,indent=2)+'\n')
prior=json.loads((previous/'native-origins.json').read_bytes())
(root/'native-origins.json').write_text(json.dumps({'perch':prior['line'],'no-blocker':prior['no-blocker']},indent=2)+'\n')
print('Prepared the same wall and blocker with the deck outside the center rays',flush=True)
