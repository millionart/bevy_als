import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;mode=sys.argv[1]
if mode=='restore':
 (r/'replay.rs').write_bytes((r/'replay.rs.final').read_bytes());(r/'inputs.json').write_bytes((r/'inputs.canonical.json').read_bytes())
elif mode=='no-construction':
 text=(r/'replay.rs.final').read_text();old='if query==0{let curves=';assert text.count(old)==1;(r/'replay.rs').write_text(text.replace(old,'if false{let curves='))
else:
 data=json.loads((r/'inputs.canonical.json').read_text())
 if mode=='post-exposure-clock':data['frames'][0]['delta_seconds']=0.03333333507180214
 elif mode=='wrong-trace-start':
  frame=next(x for x in data['frames']if x['expected_traces']);frame['expected_traces'][0]['start'][0]+=0.001
 else:raise AssertionError('Unknown VM replay control')
 (r/'inputs.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
print('Applied VM replay source/input control '+mode,flush=True)
