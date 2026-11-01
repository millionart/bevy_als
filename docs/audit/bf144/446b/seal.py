import gzip
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
gate='PASS_BOUNDED318_NATIVE_HISTORY_AND_OPTIONAL_INITIAL_POSE_WITH_LEGACY_COMPATIBILITY'
assert json.loads((r/'summary.json').read_text())['gate']==gate
for name,row in json.loads((r/'compression.json').read_text()).items():
    raw=gzip.decompress((r/(name+'.gz')).read_bytes())
    assert len(raw)==row['raw_bytes'] and hashlib.sha256(raw).hexdigest()==row['raw_sha256']
files={}
for p in sorted(r.rglob('*')):
    if not p.is_file() or p.suffix not in ['.json','.py','.ps1','.md','.gz'] or p.name in ['evidence-manifest.json','seal.report.json']:
        continue
    raw=p.read_bytes()
    decoded=gzip.decompress(raw)if p.suffix=='.gz'else raw
    assert not any(('/'+s+'/').encode()in decoded for s in ['Users','Volumes']),str(p.relative_to(r))
    subprocess.run(['git','ls-files','--error-unmatch',str(p.relative_to(Path.cwd()))],check=True,stdout=subprocess.DEVNULL,timeout=5)
    files[str(p.relative_to(r))]=dict(sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw))
(r/'evidence-manifest.json').write_text(json.dumps(dict(gate=gate,files=files,reader_migration='reader-migration.json',new_differences=[],screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Verified tracked bounded native history and initial pose evidence '+str(len(files)),flush=True)
