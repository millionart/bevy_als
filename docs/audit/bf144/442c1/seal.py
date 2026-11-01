import gzip
import hashlib
import json
import subprocess
from pathlib import Path
r=Path(__file__).resolve().parent
assert json.loads((r/'summary.json').read_text())['gate']=='PASS_REAL_PLAYGROUND_NATURAL_AND_CONTROLLED_VIEW_LIFECYCLE_AND_RESTORATION'
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file()or p.suffix not in ['.json','.py','.ps1','.md','.gz','.rs']or p.name in ['evidence-manifest.json','seal.report.json']:continue
    raw=p.read_bytes();text=(gzip.decompress(raw) if p.suffix=='.gz' else raw).decode();assert not any('/'+s+'/'in text for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}
(r/'evidence-manifest.json').write_text(json.dumps({'gate':'PASS_REAL_PLAYGROUND_NATURAL_AND_CONTROLLED_VIEW_LIFECYCLE_AND_RESTORATION','files':files,'source_actor_equivalence_claimed':False,'screenshots':0,'performance_acceptance':'paused'},indent=2)+'\n');print('Verified tracked real Playground lifecycle evidence files '+str(len(files)),flush=True)
