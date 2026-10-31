import hashlib
import json
import subprocess
from pathlib import Path
r=Path(__file__).resolve().parent
assert json.loads((r/'summary.json').read_text())['gate']=='PASS_NATIVE317_INPUT_PRODUCERS_THREE_BOUNDARIES_AND_BYTE_RESTORATION'
assert json.loads((r/'identity.report.json').read_text())['success']
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file()or p.suffix not in ['.json','.py','.ps1','.rs','.md']or p.name in ['evidence-manifest.json','seal.report.json']:continue
    raw=p.read_bytes();text=raw.decode();assert not any('/'+s+'/'in text for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}
(r/'evidence-manifest.json').write_text(json.dumps({'gate':'PASS_NATIVE317_INPUT_PRODUCERS_THREE_BOUNDARIES_AND_BYTE_RESTORATION','files':files,'full_system_equivalence':False,'screenshots':0,'performance_acceptance':'paused'},indent=2)+'\n');print('Verified Git privacy and integrity for '+str(len(files))+' direction-math evidence files',flush=True)
