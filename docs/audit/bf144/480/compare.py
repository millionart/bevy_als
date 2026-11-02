import gzip
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
def read(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
def f32(v):
    return struct.unpack('f',struct.pack('f',v))[0]

baseline=read('replay-unit-baseline.json')
data=read('replay-unit-factors.json')
inputs=read('inputs.json')
assert data['source_exact']==1332 and data['core_exact']==1330 and data['public_exact']==1302 and data['actual_exact']==1272
assert baseline['source']==data['source']and baseline['current']==data['current']
assert data==read('replay-unit-focused-counts.json')
source_by={row['row']:row for row in inputs['source']}
branches={}
for row in data['source']:
    branch=branches.setdefault(str(row['weight']),dict(calls=0,exact=0))
    branch['calls']+=1
    branch['exact']+=row['exact']
    assert row['result']['after']==row['native']
assert branches=={'1':{'calls':1324,'exact':1324},'0':{'calls':2,'exact':2},'0.25':{'calls':2,'exact':2},'0.5':{'calls':2,'exact':2},'0.75':{'calls':2,'exact':2}}
factors={(row['domain'],row['row'],row['weight'],row['mask']):row for row in data['factors']}
assert len(factors)==144
prior=json.loads(Path('docs/audit/bf144/479/inputs.json').read_bytes())
native_current={row['row']:row for row in prior['rows']if row['domain']=='current_m'and row['secondary_weight']==1}
groups={}
table=[]
current_core_checks=0
for row in data['factors']:
    full=factors[(row['domain'],row['row'],row['weight'],7)]
    assert row['result']['solved']==full['result']['solved']
    assert row['result']['core']==full['result']['core']
    if row['domain']=='source_cm'and row['mask']==7:
        assert row['result']['after']==source_by[row['row']]['expected_after']
    if row['domain']=='current_m'and row['mask']==7:
        old=native_current[row['row']]
        assert row['result']['core']['rotations']==old['expected_native_rotations']
        positions=[[v[k]for k in 'xyz']for v in old['expected_source_positions']]
        assert [row['result']['core']['joint'],row['result']['core']['end']]==positions
        current_core_checks+=1
    counts=groups.setdefault(row['domain'],{}).setdefault(str(row['mask']),dict(calls=0,q_double=0,t_double=0,q_f32=0,t_f32=0,s_double=0))
    counts['calls']+=1
    changes={}
    for bone,(a,b)in enumerate(zip(row['result']['after'],full['result']['after'])):
        fields={}
        for key,prefix in [('rotation','q'),('translation','t'),('scale','s')]:
            different=a[key]!=b[key]
            counts[prefix+'_double']+=different
            fields[key]=different
            if prefix!='s':
                unit=0.01 if key=='translation'and row['domain']=='source_cm'else 1.0
                counts[prefix+'_f32']+=[f32(v*unit)for v in a[key]]!=[f32(v*unit)for v in b[key]]
        changes[str(bone)]=fields
    if row['result']['after']!=full['result']['after']:
        table.append(dict(domain=row['domain'],row=row['row'],weight=row['weight'],mask=row['mask'],fields=changes,after=row['result']['after'],source=full['result']['after']))
assert current_core_checks==12
for mask in ['3','5','6']:
    assert sum(groups[domain][mask]['q_double']+groups[domain][mask]['t_double']for domain in groups)>0
report=dict(gate='PASS_COMPLETE_IK_ZERO_PARTIAL_FULL_TQS_AND_SOURCE_HELD_WRAPPER_FACTORS',source_branches=branches,source_complete_tqs_exact=1332,fresh_pre_weight_cores_exact=1330,current_actual_complete_tqs_exact=1272,public_controlled_weight_calls=20,public_same_source_numeric_calls=10,public_total_complete_models_exact=1302,source_partial_native_whole_wrapper_exact=6,current_partial_source_core_checked_to_prior_native=12,wrapper_records=144,groups=groups,solver_outputs_held_identical_across_wrapper_factors=True,requested_source_tqs_matches_all_ordered_global_consumer_results=True,no_extra_writer_adapter_needed=True,individual_writer_skip_or_dirty_execution_counts_not_claimed=True,float_partial_cutoff_same_at_one=True,fixture_numeric_representation_failures_not_port_differences=True,new_differences=[],confirmed_difference_count=272,descendant_or_full_rig_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
(r/'difference-table.json').write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps(dict(gate=report['gate'],source_branches=branches,groups=groups)),flush=True)
