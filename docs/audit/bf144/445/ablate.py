import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
if sys.argv[1]=='restore':
    for name,digest in json.loads((r/'final-sources.json').read_text()).items():
        raw=(r/(name+'.final')).read_bytes()
        assert hashlib.sha256(raw).hexdigest()==digest
        (r/name).write_bytes(raw)
else:
    hashes={}
    for name in ['keys.rs','observer.rs','playground_observed.rs','replay.rs']:
        raw=(r/name).read_bytes()
        hashes[name]=hashlib.sha256(raw).hexdigest()
        (r/(name+'.final')).write_bytes(raw)
        text=raw.decode()
        if name=='replay.rs':
            old='let direct=source_position+dt;'
            new='let direct=SourceSyncPlayerTick{duration,phase_duration:duration,delta_seconds:dt,play_rate:1.0,looped:true,..Default::default()}.advance_from_time(source_position).time;'
            assert text.count(old)==1
            text=text.replace(old,new)
        else:
            old='    output.sort_by_key('
            assert text.count(old)==1
            text=text.replace(old,'    output.retain(|(_,key)|matches!(key,bevy_animation_graph::core::context::node_states::StateKey::Default));\n'+old)
            if name!='keys.rs':
                old='else{c445_leaf_rows(player,&graphs,&assets)}'
                assert text.count(old)==1
                text=text.replace(old,'else{Vec::<serde_json::Value>::new()}')
        (r/name).write_text(text)
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Leaf/key and direct-second evidence variant: '+sys.argv[1],flush=True)
