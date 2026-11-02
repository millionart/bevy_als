import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');raw=(r/'reader.final.cpp').read_bytes();assert hashlib.sha256(raw).hexdigest()==json.loads((r/'reader-final-sha256.json').read_text())['sha256'];mode=sys.argv[1]
if mode=='restore':p.write_bytes(raw)
else:
 s=raw.decode()
 if mode=='zero-current':
  old='Item->SetObjectField(TEXT("current_global_transform"), CaptureTransform(Hierarchy->GetGlobalTransform(Key)));';assert s.count(old)==1;s=s.replace(old,'Item->SetObjectField(TEXT("current_global_transform"), CaptureTransform(FTransform::Identity));')
 elif mode=='no-config-guard':
  old='if (Request->HasField(TEXT("unit_boundary")))';assert s.count(old)==1;s=s.replace(old,'if ((false))')
 elif mode=='no-normal-restore':
  old='        if (OriginalBoundaryFunction != nullptr)\n        {\n            BoundaryVM->GetFunctions()[BoundaryFunctionIndex] = OriginalBoundaryFunction;\n            ActiveNativeRigUnitBoundary = nullptr;\n        }\n        TSharedRef<FJsonObject> CaseRow';assert s.count(old)==1;s=s.replace(old,'        TSharedRef<FJsonObject> CaseRow')
 else:raise ValueError(mode)
 p.write_text(s)
print('Applied exact unit boundary control '+mode,flush=True)
