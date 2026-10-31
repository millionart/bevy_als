import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
base=read('native.json')['geometry_query_probe']['samples'];actual=read('actual.native.json')['geometry_query_probe']['samples'];assert len(base)==77 and len(actual)==240
(r/'matrix-inputs.json').write_text(json.dumps({'geometry_query_probe':{'samples':base+actual}},separators=(',',':'))+'\n');print('Bound77 operator controls and240 real camera/input source-math cases separately',flush=True)
