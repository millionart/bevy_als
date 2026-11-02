import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
case=sys.argv[1]
canonical=(r/'reader.final.cpp').read_bytes()
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
if case=='restore':
    reader.write_bytes(canonical)
    print('Restored canonical readonly cache observer source bytes',flush=True)
    sys.exit(0)
specs={
    'global-false':(b'Cache->SetBoolField(TEXT("current_global_dirty"), CacheElement->GetDirtyState().IsDirty(ERigTransformType::CurrentGlobal));',b'Cache->SetBoolField(TEXT("current_global_dirty"), false);'),
    'global-identity':(b'Cache->SetObjectField(TEXT("current_global_cache"), CaptureTransform(CacheElement->GetTransform().Get(ERigTransformType::CurrentGlobal)));',b'Cache->SetObjectField(TEXT("current_global_cache"), CaptureTransform(FTransform::Identity));')
}
old,new=specs[case]
assert canonical.count(old)==1
variant=canonical.replace(old,new)
(r/('reader.'+case+'.cpp')).write_bytes(variant)
reader.write_bytes(variant)
(r/('ablation-source-'+case+'.json')).write_text(json.dumps(dict(case=case,sha256=hashlib.sha256(variant).hexdigest(),only_metadata_changed=True),indent=2)+'\n')
print('Applied raw-cache metadata-only ablation '+case,flush=True)
