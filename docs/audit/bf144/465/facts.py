import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);units=[('Engine/Source/Runtime/Engine/Classes/Kismet/KismetMathLibrary.h',[(110,132)]),('Engine/Source/Runtime/Engine/Private/KismetMathLibrary.cpp',[(429,465)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Simulation/RigUnit_SpringInterp.cpp',[(62,86)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h',[(1550,1561),(1670,1690),(1702,1712),(1740,1758)]),('src/control_rig/control_rig_units.rs',[(455,519)]),('src/control_rig/control_rig_math.rs',[(86,145)])];facts=[]
for key,ranges in units:
 p=engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured complete actual critical spring source chain, zero delta lifetime and centimeter versus meter arithmetic domain',flush=True)
