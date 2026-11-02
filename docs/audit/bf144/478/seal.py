import gzip
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
gate='PASS_ACTUAL_IK_POSITION_NATIVE_PUBLIC_FACTORS_AND_EXISTING_D249'
assert json.loads((r/'summary.json').read_text())['gate']==gate
summary=json.loads((r/'summary.json').read_text())
for row in summary['external_source_files']:
    p=Path(row['path']);raw=p.read_bytes()
    assert len(raw)==row['bytes']and hashlib.sha256(raw).hexdigest()==row['sha256']
    subprocess.run(['git','ls-files','--error-unmatch',row['path']],check=True,stdout=subprocess.DEVNULL,timeout=5)
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file()or p.suffix not in ['.json','.py','.ps1','.md','.rs','.final','.gz','.txt']or p.name in ['evidence-manifest.json','seal.report.json']:
        continue
    raw=p.read_bytes()
    decoded=gzip.decompress(raw)if p.suffix=='.gz'else raw
    assert not any(('/'+s+'/').encode()in decoded for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]=dict(sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw))
(r/'evidence-manifest.json').write_text(json.dumps(dict(gate=gate,files=files,full_root_or_als_equivalence_claimed=False,new_differences=[],screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Verified tracked actual native/public position factors and byte-restored arithmetic evidence '+str(len(files)),flush=True)
