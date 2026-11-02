import gzip
import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
case=sys.argv[1]
if case=='restore':
    for name in ['replay.rs','inputs.json']:
        (r/name).write_bytes(raw(name+'.final'))
    print('Restored complete raw-cache model and input source bytes',flush=True)
    sys.exit(0)
specs={
    'propagation':('        self.propagate(index);',''),
    'local-dirty':('        self.nodes[index].dirty=1;','        self.nodes[index].dirty=0;'),
    'source-normalizer':('pub fn normalized(q:[f64;4])->[f64;4] {normalize(q,8)}','pub fn normalized(q:[f64;4])->[f64;4] {normalize(q,0)}')
}
full=json.loads(raw('inputs.json.final'))
inputs=dict(full)
inputs['source']=full['source'][:1]
inputs['current']=[row for row in full['current']if row['kind']=='clean_control']
assert len(inputs['source'])==len(inputs['current'])==1
data=json.dumps(inputs,separators=(',',':'))+'\n'
source=raw('replay.rs.final').decode()
before,after=specs[case]
assert source.count(before)==1
source=source.replace(before,after)
(r/('negative-inputs-'+case+'.json')).write_text(data)
(r/('replay.no-'+case+'.rs')).write_text(source)
(r/'inputs.json').write_text(data)
(r/'replay.rs').write_text(source)
(r/('negative-selection-'+case+'.json')).write_text(json.dumps(dict(case=case,source_cases=1,current_clean_clone_controls=1,source_sha256=hashlib.sha256(source.encode()).hexdigest(),inputs_sha256=hashlib.sha256(data.encode()).hexdigest()),indent=2)+'\n')
print('Prepared minimal raw-cache and consumer negative '+case,flush=True)
