import sys
from pathlib import Path

reader = Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
canonical = Path('tools/tmp/bf144/491/reader.final.cpp').read_bytes()
name = sys.argv[1]
if name=='restore':
    reader.write_bytes(canonical)
else:
    initial = b'FootSettingsGeneral->ContainerPtrToValuePtr<void>(FootSettingsOverride), bInitialFootIkBones);'
    frame = b'FootSettingsGeneral->ContainerPtrToValuePtr<void>(FootSettingsOverride), bUseFootIkBones);'
    if name=='no-initial-write':
        token = initial
    else:
        assert name=='no-frame-write'
        token = frame
    needle = b'UseFootIkBonesProperty->SetPropertyValue_InContainer(\n'
    start = canonical.rfind(needle,0,canonical.index(token))
    end = canonical.index(token)+len(token)
    assert start>=0 and canonical.count(token)==1
    reader.write_bytes(canonical[:start]+b'FMath::IsFinite(0.0f);'+canonical[end:])
print(f'Reader source variant {name}',flush=True)
