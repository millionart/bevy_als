import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;mode=sys.argv[1];expected=json.loads((r/'final-sources.json').read_text())
for name,digest in expected.items():
 saved=(r/(name+'.final')).read_bytes();assert hashlib.sha256(saved).hexdigest()==digest
 if mode=='restore':(r/name).write_bytes(saved);continue
 text=saved.decode()
 if mode=='no-observation'and name!='rag_observer.rs':
  old='        "ragdoll_blend":if before{Vec::new()}else{c458_ragdoll_blend(player,&graphs,&skeletons,&assets)},';assert text.count(old)==1;text=text.replace(old,'')
 elif mode=='constant-state':
  old='"weights":weights,"last_active_index":active';assert text.count(old)==1;text=text.replace(old,'"weights":vec![1.0,0.0],"last_active_index":active')
 else:assert mode=='no-observation'
 (r/name).write_text(text)
print('Applied Ragdolling observation source control '+mode,flush=True)
