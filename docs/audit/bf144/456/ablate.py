import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;mode=sys.argv[1];expected=json.loads((r/'final-sources.json').read_text())
for name,digest in expected.items():
    saved=(r/(name+'.final')).read_bytes();assert hashlib.sha256(saved).hexdigest()==digest
    if mode=='restore':(r/name).write_bytes(saved)
    elif name!='rig_observer.rs':
        text=saved.decode()
        if mode=='no-observation':
            old='        "rig_snapshot":c456_rig_snapshot(player,&graphs,&skeletons,&assets,before),';assert text.count(old)==1;text=text.replace(old,'')
        else:
            assert mode=='no-capture';old='    app.init_resource::<C456Probe>();';assert text.count(old)==(1 if name=='playground_observed.rs'else 0)
            if name=='playground_observed.rs':text=text.replace(old,old+'\n    app.insert_resource(bevy_als::foot_ik_apply::AlsFootIkTelemetryEnabled(false));')
        (r/name).write_text(text)
print('Applied actual Rig observer source control '+mode,flush=True)
