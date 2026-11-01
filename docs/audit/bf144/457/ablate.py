import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');saved=(r/'reader.final.cpp').read_bytes()
assert hashlib.sha256(saved).hexdigest()==json.loads((r/'reader-final-sha256.json').read_text())['sha256']
mode=sys.argv[1]
if mode=='restore':p.write_bytes(saved)
else:
    text=saved.decode();old,new={
        'no-output-hook':('TargetInput->InputPose.SetLinkNode(&RigOutputObservation);',''),
        'constant-action':('const FName Name = ActionNameProperty->GetPropertyValue_InContainer(Memory);','const FName Name = NAME_None;'),
        'no-parent-guard':('if (bCaptureRigOutput && !bCapturePoseBoundaries) { return nullptr; }',''),
    }[mode]
    assert text.count(old)==1;p.write_text(text.replace(old,new))
print('Applied source Rig-output capability control '+mode,flush=True)
