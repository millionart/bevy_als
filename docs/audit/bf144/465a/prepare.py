import copy
import json
from pathlib import Path

r=Path(__file__).resolve().parent;request=json.loads(Path('docs/audit/bf144/462/request.json').read_text());request['capture_float_spring_state']=True;(r/'request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
bad=copy.deepcopy(request);bad['cases']=bad['cases'][:1];bad['cases'][0]['frames']=bad['cases'][0]['frames'][:1];bad['capture_float_spring_state']='true';(r/'bad-type.request.json').write_text(json.dumps(bad,separators=(',',':'))+'\n');print('Prepared actual two319 histories and strict native spring state flag control',flush=True)
