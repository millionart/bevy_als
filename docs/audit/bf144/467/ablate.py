import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');mode=sys.argv[1];prototype=mode in['zero-global','restore-prototype'];raw=(r/('reader.with-local.cpp'if prototype else'reader.final.cpp')).read_bytes();assert hashlib.sha256(raw).hexdigest()==json.loads((r/('reader-with-local-sha256.json'if prototype else'reader-final-sha256.json')).read_text())['sha256']
if mode in['restore','restore-prototype']:p.write_bytes(raw)
else:
 s=raw.decode()
 if mode=='zero-global':
  old='Bone->SetObjectField(TEXT("current_global_transform"), CaptureTransform(Hierarchy->GetGlobalTransform(Key)));';assert s.count(old)==1;s=s.replace(old,'Bone->SetObjectField(TEXT("current_global_transform"), CaptureTransform(FTransform::Identity));')
 elif mode=='no-type-guard':
  old='if (Request->HasField(TEXT("capture_current_hierarchy")) &&\n        (!Request->HasTypedField<EJson::Boolean>(TEXT("capture_current_hierarchy")) || !Request->TryGetBoolField(TEXT("capture_current_hierarchy"), bCaptureCurrentHierarchy))) { return nullptr; }';assert s.count(old)==1;s=s.replace(old,'')
 else:raise ValueError(mode)
 p.write_text(s)
print('Applied current hierarchy capture control '+mode,flush=True)
