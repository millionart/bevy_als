import gzip
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent;summary=json.loads((r/'foundation-summary.json').read_bytes());assert summary['gate']=='PASS_NATIVE_IK_TEMPORAL_ABI_DATA_FOUNDATION_WITH_COMPLETE_HISTORY_TRANSPARENCY'
for row in summary['external_source_files']:
 p=Path(row['path']);raw=p.read_bytes();assert len(raw)==row['bytes']and hashlib.sha256(raw).hexdigest()==row['sha256'];subprocess.run(['git','ls-files','--error-unmatch',row['path']],check=True,stdout=subprocess.DEVNULL,timeout=5)
files={}
for p in sorted(r.rglob('*')):
 if not p.is_file()or p.suffix not in['.json','.py','.ps1','.md','.rs','.final','.gz','.txt']or p.name in['foundation-manifest.json','seal_foundation.report.json']:continue
 raw=p.read_bytes();decoded=gzip.decompress(raw)if p.suffix=='.gz'else raw;assert not any(('/'+x+'/').encode()in decoded for x in['Users','Volumes']),p.name;subprocess.run(['git','ls-files','--error-unmatch',str(p)],check=True,stdout=subprocess.DEVNULL,timeout=5);files[p.name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
(r/'foundation-manifest.json').write_text(json.dumps(dict(gate=summary['gate'],files=files,final_parent_gate_pending=True,screenshots=0,performance_acceptance='paused'),indent=2)+'\n');print('Verified Git-tracked native IK foundation files '+str(len(files)),flush=True)
