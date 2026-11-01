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
    for name in ['observer.rs','playground_observed.rs','replay.rs']:
        raw=(r/name).read_bytes()
        hashes[name]=hashlib.sha256(raw).hexdigest()
        (r/(name+'.final')).write_bytes(raw)
        text=raw.decode()
        if name=='playground_observed.rs':
            start=text.index('    app.add_systems(PostUpdate,(|| true).pipe(c444_animation)')
            end=text.index('    if let AppExit::Error(code) = app.run() {',start)
            text=text[:start]+text[end:]
        elif name=='replay.rs':
            old='if index==0{"Forced(Absolute(0.0))".to_string()}else{format!("Forced(Delta({:?}))",scalar(b,"animation_delta"))}'
            assert text.count(old)==1
            text=text.replace(old,'format!("Forced(Delta({:?}))",scalar(b,"animation_delta"))')
        (r/name).write_text(text)
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Animation input and first clock evidence variant: '+sys.argv[1],flush=True)
