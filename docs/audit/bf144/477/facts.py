import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Highlevel/Hierarchy/RigUnit_TwoBoneIKSimple.cpp',[(111,221)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Math/ControlRigMathLibrary.cpp',[(6,98)]),('Engine/Source/Runtime/Core/Public/Math/Vector.h',[(1575,1586),(1714,1733)]),('src/control_rig/control_rig_units.rs',[(322,441)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',[(1054,1065),(1190,1238)])];facts=[]
for key,ranges in specs:
 p=engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured fresh native auto-length wrapper/vector arithmetic and existing native solve consumer',flush=True)
