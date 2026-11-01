import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
lines=p.read_text().splitlines()
result=[dict(key=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in [(4480,4509),(4523,4537),(4610,4648)]])]
(r/'source-facts.json').write_text(json.dumps(result,indent=2)+'\n')
print('Captured bounded frame and controlled source spawn/floor contracts',flush=True)
