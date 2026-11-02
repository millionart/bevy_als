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
    print('Restored complete canonical wrapper source and input bytes',flush=True)
    sys.exit(0)
bit={'slerp':1,'normalizer':2,'vector':4}[case]
full=json.loads(raw('inputs.json.final'))
factors=json.loads(raw('replay-unit-factors.json'))['factors']
complete={(row['domain'],row['row'],row['weight']):row for row in factors if row['mask']==7}
candidate=next(row for row in factors if row['domain']=='source_cm'and row['mask']==7-bit and row['result']['after']!=complete[(row['domain'],row['row'],row['weight'])]['result']['after'])
weight=candidate['weight']
source=[row for row in full['source']if row['weight']==weight]
assert len(source)==2 and all(0<row['weight']<1 for row in source)
indices={row['row']for row in source}
current=[row for row in full['current']if row['kind']=='source_common'and row['row']in indices]
assert len(current)==2
data=json.dumps(dict(source=source,current=current),separators=(',',':'))+'\n'
canonical=raw('replay.rs.final').decode()
needle='fn evaluate(row:&Value,source_solver:bool,mask:u8)->Value {\n'
assert canonical.count(needle)==1
source_code=canonical.replace(needle,needle+'    let mask=mask&!'+str(bit)+'u8;\n')
(r/('negative-inputs-'+case+'.json')).write_text(data)
(r/('replay.no-'+case+'.rs')).write_text(source_code)
(r/'inputs.json').write_text(data)
(r/'replay.rs').write_text(source_code)
(r/('negative-selection-'+case+'.json')).write_text(json.dumps(dict(case=case,bit=bit,source_rows=sorted(indices),weight=weight,source_solver_unchanged=True,source_sha256=hashlib.sha256(source_code.encode()).hexdigest(),inputs_sha256=hashlib.sha256(data.encode()).hexdigest()),indent=2)+'\n')
print('Prepared minimal two-leg partial native wrapper negative '+case,flush=True)
