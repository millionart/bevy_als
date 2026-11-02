import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
specs=[('Engine/Source/Runtime/AnimationCore/Private/TwoBoneIK.cpp',[(70,183)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Math/ControlRigMathLibrary.cpp',[(6,20)]),('Engine/Source/Runtime/Core/Public/Math/Vector.h',[(1545,1551),(1714,1730),(1765,1777),(2058,2074)]),('src/control_rig/control_rig_ik.rs',[(18,25),(72,170)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',[(1190,1238)])]
facts=[]
for key,ranges in specs:
    p=engine/key if key.startswith('Engine/')else Path(key)
    raw=p.read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh source position solver, vector scalar arithmetic and exact current geometry implementation',flush=True)
