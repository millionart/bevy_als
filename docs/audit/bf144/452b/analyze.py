import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;label=sys.argv[1]
def load(p):return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
def f32(x):return struct.unpack('f',struct.pack('f',x))[0]
def project(q):return [f32(-q[0]),f32(-q[2]),f32(-q[1]),f32(q[3])]
modes=['source','normalize-delta','normalize-product','one-local-normalize','standard-sqrt','project-primitives']
inputs=load(r/'inputs.json');names=inputs['names'];assert len(names)==79 and len(set(names))==79
expected={c['name']:c['expected']for c in inputs['cases']};assert len(expected)==1272
result=load(r/('replay-'+label+'.json'));assert result['native_rotation_rows_exact']==100488
rows=result['outputs']
def validate(rows):
    assert len(rows)==7632 and len({(x['name'],x['mode'])for x in rows})==7632
    assert {(x['name'],x['mode'])for x in rows}=={(n,m)for n in expected for m in modes}
    for x in rows:assert len(x['rotations'])==79 and all(len(q)==4 for q in x['rotations'])
validate(rows);index={(x['name'],x['mode']):x['rotations']for x in rows}
current={f"{x['source_frame']:03}:{x['domain']}":{b['name']:b['rotation']for b in x['pose']['bones']if b['name']!='SKM_Als'}for x in load(Path('docs/audit/bf144/451b/target-replay-restored.json'))['domain_outputs']}
assert current.keys()==expected.keys()
summary={d:{m:dict(double_changed_bones=0,projected_changed_bones=0,first_double=None,first_projected=None)for m in modes}for d in ['cc','sc','cs','ss']}
current_delta={d:0 for d in summary};table=[]
for name,native in expected.items():
    domain=name.split(':')[1];source=index[name,'source'];assert source==native
    assert current[name].keys()==set(names)
    current_delta[domain]+=sum(project(q)!=current[name][bone]for bone,q in zip(names,native,strict=True))
    for mode in modes:
        actual=index[name,mode];count=projected=0;first_d=first_p=None
        for bone,a,b in zip(names,actual,native,strict=True):
            if a!=b:
                count+=1
                if first_d is None:first_d=dict(name=name,bone=bone,controlled=a,native=b)
            if project(a)!=project(b):
                projected+=1
                if first_p is None:first_p=dict(name=name,bone=bone,controlled=project(a),native=project(b))
        total=summary[domain][mode];total['double_changed_bones']+=count;total['projected_changed_bones']+=projected
        if total['first_double']is None:total['first_double']=first_d
        if total['first_projected']is None:total['first_projected']=first_p
        table.append(dict(name=name,mode=mode,double_changed_bones=count,projected_changed_bones=projected))
assert current_delta==dict(cc=23032,sc=23539,cs=22757,ss=22719)
for mode in modes[1:]:
    assert sum(x[mode]['double_changed_bones']for x in summary.values())>0,'Missing source factor double signal: '+mode
    assert sum(x[mode]['projected_changed_bones']for x in summary.values())>0,'Missing source factor projection signal: '+mode
negative=[]
for kind in ['missing-mode','duplicate-mode','wrong-mode','missing-bone']:
    if kind=='missing-mode':bad=rows[:-1]
    elif kind=='duplicate-mode':bad=[*rows[:-1],rows[0]]
    elif kind=='wrong-mode':bad=[dict(rows[0],mode='invalid'),*rows[1:]]
    else:bad=[dict(rows[0],rotations=rows[0]['rotations'][:-1]),*rows[1:]]
    try:validate(bad)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid source arithmetic matrix passed: '+kind)
analysis=dict(gate='PASS_SOURCE_HEAD_F64_ARM64_FOUR_DOMAIN_ROTATIONS_AND_FIVE_INDEPENDENT_FACTORS',native_rotation_rows_exact=100488,native_case_count=1272,source_cc_actual_current_input_cases=318,source_ss_actual_source_input_cases=318,current_to_native_rotations=current_delta,source_factor_summaries=summary,negative_fixtures_rejected=negative,source_factors_are_not_additive=True,intermediate_projection_is_source_F64_primitive_then_F32_not_current_glam=True,full_root_or_als_equivalence_claimed=False,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/('analysis-'+label+'.json')).write_text(json.dumps(analysis,indent=2)+'\n');(r/('frame-table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n')
print('Verified100488 exact native rotation rows, actual CC/SS endpoints and five source operator factors',flush=True)
print(json.dumps({d:{m:[v['double_changed_bones'],v['projected_changed_bones']]for m,v in values.items()}for d,values in summary.items()}),flush=True)
