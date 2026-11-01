import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);facts=[]
for key,ranges in [('Engine/Source/Runtime/Engine/Classes/Kismet/KismetMathLibrary.h',[(110,132)]),('Engine/Source/Runtime/Engine/Private/KismetMathLibrary.cpp',[(429,450),(451,465)])]:
 p=engine/key;raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
key='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp';raw=(r/'reader.final.cpp').read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in [(275,323),(5118,5131),(5310,5323)]]));(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured non-reflected actual native members and strict opt-in typed array capture without changing reflection fields',flush=True)
