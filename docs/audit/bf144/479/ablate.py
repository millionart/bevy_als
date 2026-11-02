import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
case=sys.argv[1]
canonical=(r/'replay.rs.final').read_text()
full=json.loads(raw('inputs.json.final'))
if case=='restore':
    (r/'replay.rs').write_bytes((r/'replay.rs.final').read_bytes())
    (r/'inputs.json').write_bytes(raw('inputs.json.final'))
    print('Restored canonical source and all5192 input bytes',flush=True)
    sys.exit(0)
specs={
    'dot':(30,'if mask&1!=0 {(a.x*b.x+a.y*b.y)+a.z*b.z}else{(a.x*b.x+a.z*b.z)+a.y*b.y}','(a.x*b.x+a.z*b.z)+a.y*b.y'),
    'vector-normal':(29,'if mask&2!=0 {v*(1.0/square.sqrt())}else{v/square.sqrt()}','v/square.sqrt()'),
    'quat-normal':(23,'if mask&8!=0 {quaternion::source_normalize(v)}else{project_q(rig_normalize(DQuat::from_array(project_q(v))).to_array())}','project_q(rig_normalize(DQuat::from_array(project_q(v))).to_array())'),
    'product':(15,'if mask&16!=0 {(DQuat::from_array(a)*DQuat::from_array(b)).to_array()}\n    else{project_q((DQuat::from_array(project_q(a))*DQuat::from_array(project_q(b))).to_array())}','project_q((DQuat::from_array(project_q(a))*DQuat::from_array(project_q(b))).to_array())')
}
if case=='rotate':
    start=canonical.index('fn rotate(')
    end=canonical.index('fn between(',start)
    old=canonical[start:end]
    specs[case]=(27,old,'fn rotate(rotation:Q,v:DVec3,_mask:u8)->DVec3 {project(DQuat::from_array(project_q(rotation))*project(v))}\n')
mask,old,new=specs[case]
assert canonical.count(old)==1
result=json.loads(raw('replay-unit-factors.json'))
def float_different(row):
    f=lambda values:[struct.unpack('f',struct.pack('f',v))[0]for v in values]
    return any(f(a)!=f(b)for a,b in zip(row['result']['rotations'],row['native']))
candidates=[row for row in result['outputs']if row['mask']==mask and not row['exact']]
assert candidates
floating=[row for row in candidates if float_different(row)]
selected=min(floating or candidates,key=lambda row:(row['domain']!='current_m',row['secondary_weight']!=1,row['row']))
index=selected['row']-selected['row']%2
pair=full['rows'][index:index+2]
assert len(pair)==2 and [row['secondary_weight']for row in pair]==[1,0]
assert pair[0]['input_index']==pair[1]['input_index']
assert full['rows'][selected['row']]['expected_native_rotations']==selected['native']
raw=json.dumps(dict(rows=pair),separators=(',',':'))+'\n'
source=canonical.replace(old,new)
(r/('negative-inputs-'+case+'.json')).write_text(raw)
(r/('replay.no-'+case+'.rs')).write_text(source)
(r/'inputs.json').write_text(raw)
(r/'replay.rs').write_text(source)
record=dict(case=case,orientation_factor_mask=mask,original_full_input_row=index,selected_row=selected['row'],domain=selected['domain'],selected_secondary_weight=selected['secondary_weight'],f32_counterexample=float_different(selected),pair_qualification_from_full5192=True,positions_preserved=True,source_sha256=hashlib.sha256(source.encode()).hexdigest(),inputs_sha256=hashlib.sha256(raw.encode()).hexdigest())
(r/('negative-selection-'+case+'.json')).write_text(json.dumps(record,indent=2)+'\n')
print('Prepared minimal two-input native orientation negative '+case+' at full input '+str(selected['row']),flush=True)
