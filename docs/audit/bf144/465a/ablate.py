import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');raw=(r/'reader.final.cpp').read_bytes();assert hashlib.sha256(raw).hexdigest()==json.loads((r/'reader-final-sha256.json').read_text())['sha256'];mode=sys.argv[1]
if mode=='restore':p.write_bytes(raw)
else:
 s=raw.decode()
 if mode=='zero-state':
  for old,new in [('Spring->PrevTarget','0.0f'),('Spring->Velocity','0.0f'),('Spring->bPrevTargetValid','false')]:assert s.count(old)==1;s=s.replace(old,new)
 elif mode=='no-type-guard':
  old='if (Request->HasField(TEXT("capture_float_spring_state")) &&\n        (!Request->HasTypedField<EJson::Boolean>(TEXT("capture_float_spring_state")) || !Request->TryGetBoolField(TEXT("capture_float_spring_state"), bCaptureFloatSpringState))) { return nullptr; }';assert s.count(old)==1;s=s.replace(old,'')
 else:raise ValueError(mode)
 p.write_text(s)
print('Applied non-reflected float spring state control '+mode,flush=True)
