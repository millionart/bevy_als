import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');raw=(r/'reader.final.cpp').read_bytes();assert hashlib.sha256(raw).hexdigest()==json.loads((r/'reader-final-sha256.json').read_text())['sha256']
mode=sys.argv[1]
if mode=='restore':p.write_bytes(raw)
else:
 text=raw.decode()
 if mode=='no-vm-transform':
  for old,new in [('bReferenceRayVmSpace ? RigContext.ToWorldSpace(Start) : Start','Start'),('bReferenceRayVmSpace ? RigContext.ToWorldSpace(End) : End','End')]:
   assert text.count(old)==1;text=text.replace(old,new)
 elif mode=='no-parent-guard':
  old='(bReferenceRayVmSpace && !bCaptureWorld)';assert text.count(old)==1;text=text.replace(old,'false')
 else:raise ValueError(mode)
 p.write_text(text)
print('Applied native reference ray capability control '+mode,flush=True)
