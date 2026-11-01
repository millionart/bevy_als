import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime/Engine/Private/Animation/AnimInstanceProxy.cpp'
units=[(engine,'source:UE/Engine/Private/Animation/AnimInstanceProxy.cpp',[(1331,1377)])]
p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
lines=p.read_text().splitlines()
indices=[next(i for i,l in enumerate(lines)if 'bool bAdvanceBeforeFirstWorldTick' in l),next(i for i,l in enumerate(lines)if 'if (bAdvanceBeforeFirstWorldTick)' in l),next(i for i,l in enumerate(lines)if 'World->Tick(LEVELTICK_All' in l)]
units.append((p,str(p),[(i+1,i+8)for i in indices]))
result=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    result.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(result,indent=2)+'\n')
print('Captured callback gating and a counter-only first-world phase control',flush=True)
