import gzip
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
gate='PASS_CURRENT_PHYSICS_TO_ANIMATION_INPUT_AND_CLOCK_ENTRY_WITH_HISTORY_BOUNDARY'
assert json.loads((r/'summary.json').read_text())['gate']==gate
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file() or p.suffix not in ['.json','.py','.ps1','.md','.rs','.final','.gz'] or p.name in ['evidence-manifest.json','seal.report.json']:
        continue
    raw=p.read_bytes()
    decoded=gzip.decompress(raw) if p.suffix=='.gz' else raw
    assert not any(('/'+s+'/').encode() in decoded for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]=dict(sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw))
(r/'evidence-manifest.json').write_text(json.dumps(dict(gate=gate,files=files,full_root_equivalence_claimed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Verified tracked current animation input evidence files '+str(len(files)),flush=True)
