import hashlib
import json
import subprocess
from pathlib import Path
r=Path(__file__).resolve().parent
assert json.loads((r/'summary.json').read_text())['gate']=='PASS_VIEW_SELECTION_PHASE_AND_ACTUAL240_DIRECTION_REPLAY'
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file()or p.suffix not in ['.json','.py','.ps1','.rs','.md']or p.name in ['evidence-manifest.json','seal.report.json']:continue
    raw=p.read_bytes();text=raw.decode();assert not any('/'+s+'/'in text for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],stdout=subprocess.DEVNULL,check=True,timeout=5)
    files[str(p.relative_to(r))]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}
assert not list(r.rglob('*.png'))
(r/'evidence-manifest.json').write_text(json.dumps({'gate':'PASS_VIEW_SELECTION_PHASE_AND_ACTUAL240_DIRECTION_REPLAY','files':files,'full_input_math_equivalence':False,'screenshots':0,'performance_acceptance':'paused'},indent=2)+'\n');print('Verified Git tracking privacy and byte identity for '+str(len(files))+' evidence files',flush=True)
