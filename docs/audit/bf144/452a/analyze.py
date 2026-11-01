import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
def load(p):return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
def f32(x):return struct.unpack('f',struct.pack('f',x))[0]
def rotations(p):return {b['name']:b['rotation']for b in p['bones']if b['name']!='SKM_Als'}
def native_rotations(p):
    return {b['name']:[f32(-b['local_transform']['rotation']['x']),f32(-b['local_transform']['rotation']['z']),f32(-b['local_transform']['rotation']['y']),f32(b['local_transform']['rotation']['w'])]for b in p['bones']}
def key(row):return ''.join(str(int(row[k]))for k in ['delta_normalize','product_normalize','second_local_normalize'])
def difference(a,b):
    assert a.keys()==b.keys()
    raw=aligned=0;first=None
    for name in sorted(a):
        x=a[name];y=b[name]
        if x!=y:
            raw+=1
            if first is None:first=dict(bone=name,a=x,b=y)
        if sum(p*q for p,q in zip(x,y))<0:x=[-p for p in x]
        aligned+=x!=y
    return dict(rotation_bones_raw=raw,rotation_bones_aligned=aligned,first=first)
def validate(rows):
    assert len(rows)==2544 and len({(x['source_frame'],key(x))for x in rows})==2544
    assert {key(x)for x in rows}=={f'{a}{b}{c}'for a in [0,1]for b in [0,1]for c in [0,1]}
    for row in rows:
        assert row['target_query']==row['source_frame']+1
        bones=rotations(row['pose']);assert len(bones)==79 and bones.keys()==names
        assert len(row['pose']['bones'])==80
rows=load(r/('replay-'+label+'.json'))
assert rows['actual_public_frames_exact']==318 and rows['actual_rotation_bones_exact']==25440
native={}
for batch in load(Path('docs/audit/bf144/451b/requests.json')):
    record=load(Path('docs/audit/bf144/451b')/(batch['label']+'.native.json'))
    for row in record['mesh_additive_probe']['samples']:
        i,mode=row['name'].split(':')
        if mode=='cc':native[int(i)]=native_rotations(row['pose'])
assert len(native)==318;names=native[0].keys();validate(rows['outputs'])
index={(x['source_frame'],key(x)):x for x in rows['outputs']}
input_rows=load(Path('docs/audit/bf144/451b/replay-inputs.json'))['frames']
variants=sorted({key(x)for x in rows['outputs']});effects={}
for axis,name in enumerate(['delta_normalize','product_normalize','second_local_normalize']):
    for v in variants:
        if v[axis]!='0':continue
        w=v[:axis]+'1'+v[axis+1:];effects[v+'-'+w]=dict(axis=name,rotation_bones_raw=0,rotation_bones_aligned=0,first=None)
summary={v:dict(rotation_bones_raw=0,rotation_bones_aligned=0,first=None)for v in variants}
table=[]
def add(total,d,i):
    for k in ['rotation_bones_raw','rotation_bones_aligned']:total[k]+=d[k]
    if total['first']is None and d['first']is not None:total['first']=dict(source_frame=i,**d['first'])
for i in range(318):
    values={v:rotations(index[i,v]['pose'])for v in variants}
    for v in variants:
        d=difference(values[v],native[i]);add(summary[v],d,i)
        table.append(dict(source_frame=i,variant=v,native_comparison=d))
        public={b['name']:b for b in input_rows[i]['current_output']['bones']}
        for b in index[i,v]['pose']['bones']:
            for channel in ['translation','scale','weights']:assert b[channel]==public[b['name']][channel]
    assert values['110']==rotations(input_rows[i]['current_output'])
    for edge,total in effects.items():
        a,b=edge.split('-');add(total,difference(values[a],values[b]),i)
assert summary['110']['rotation_bones_raw']==23032
for name in ['delta_normalize','product_normalize','second_local_normalize']:
    assert sum(x['rotation_bones_raw']for x in effects.values()if x['axis']==name)>0,'Missing factor signal: '+name
negative=[]
for kind in ['missing-variant','duplicated-variant','wrong-frame','wrong-bone']:
    bad=rows['outputs'][:-1]
    if kind=='duplicated-variant':bad=rows['outputs'][:-1]+[rows['outputs'][0]]
    elif kind=='wrong-frame':bad=[dict(rows['outputs'][0],target_query=999),*rows['outputs'][1:]]
    elif kind=='wrong-bone':
        first=json.loads(json.dumps(rows['outputs'][0]));first['pose']['bones'][0]['name']='invalid';bad=[first,*rows['outputs'][1:]]
    try:validate(bad)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid stage matrix accepted: '+kind)
result=dict(gate='PASS_CURRENT318_HEAD_ROTATION_STAGE_MODEL_AND_FULL_EIGHT_FACTOR_MATRIX',actual_frames_exact=318,current_rotation_rows_exact=25440,source_bone_rows_per_variant=25122,variant_count=8,variants_to_native=summary,factor_edges=effects,negative_fixtures_rejected=negative,all_non_rotation_channels_unchanged=True,full_source_math_or_root_equivalence_claimed=False,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/('analysis-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n')
(r/('frame-table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps({k:v for k,v in result.items()if k not in ['variants_to_native','factor_edges']}),flush=True)
print(json.dumps({v:x['rotation_bones_raw']for v,x in summary.items()}),flush=True)
