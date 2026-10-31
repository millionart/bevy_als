import hashlib
import json
import subprocess
from pathlib import Path
r=Path(__file__).resolve().parent
assert json.loads((r/'summary.json').read_text())['gate']=='PASS_INDEPENDENT_SOURCE_TARGET240_PARAMETERS_AND_ACTUAL_PLAN_CONSUMERS'
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file()or p.suffix not in ['.json','.py','.ps1','.rs','.md']or p.name in ['evidence-manifest.json','seal.report.json']:continue
    raw=p.read_bytes();text=raw.decode('utf-8');assert not any('/'+part+'/'in text for part in ['Users','Volumes']),str(p.relative_to(r))
    relative=str(p.relative_to(Path.cwd()));subprocess.run(['git','ls-files','--error-unmatch',relative],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}
assert not list(r.rglob('*.png'))
(r/'evidence-manifest.json').write_text(json.dumps({'format':'tracked_audit_evidence_v1','gate':'PASS_INDEPENDENT_SOURCE_TARGET240_PARAMETERS_AND_ACTUAL_PLAN_CONSUMERS','files':files,'screenshots':0,'performance_acceptance':'paused','full_system_equivalence':False},indent=2)+'\n');print('Verified Git tracking privacy and integrity for '+str(len(files))+' evidence files',flush=True)
