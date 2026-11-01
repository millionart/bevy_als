import gzip
import hashlib
import json
import subprocess
from pathlib import Path
r=Path(__file__).resolve().parent
assert json.loads((r/'summary-a.json').read_text())['gate']=='PASS_CURRENT240_HORIZONTAL_PHYSICS_AND_MATCHED_SOURCE_HISTORY_WITH_VERTICAL_DEFERRED'
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file()or p.suffix not in ['.json','.py','.ps1','.md','.gz','.rs','.final']or p.name in ['evidence-manifest-a.json','seal_a.report.json']:continue
    raw=p.read_bytes();text=(gzip.decompress(raw) if p.suffix=='.gz' else raw).decode();assert not any('/'+s+'/'in text for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}
(r/'evidence-manifest-a.json').write_text(json.dumps({'gate':'PASS_CURRENT240_HORIZONTAL_PHYSICS_AND_MATCHED_SOURCE_HISTORY_WITH_VERTICAL_DEFERRED','files':files,'source_actor_equivalence_claimed':False,'screenshots':0,'performance_acceptance':'paused'},indent=2)+'\n');print('Verified tracked horizontal physics phase evidence files '+str(len(files)),flush=True)
