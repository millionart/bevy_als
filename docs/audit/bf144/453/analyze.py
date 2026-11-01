import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;label=sys.argv[1]
def load(p):return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
data=load(r/('replay-'+label+'.json'));assert data['native_rows_exact']==data['current_rows_exact']==100488
rows=data['outputs'];modes=['source','early-unit','current']
inputs=load(r/'inputs.json');names=inputs['names'];assert len(names)==79 and len(set(names))==79
expected={x['name']for x in inputs['cases']}
def validate(rows):
    assert len(rows)==len(expected)*len(modes)and len({(x['name'],x['mode'])for x in rows})==len(rows)
    assert {(x['name'],x['mode'])for x in rows}=={(n,m)for n in expected for m in modes}
    for x in rows:
        assert len(x['bones'])==79
        for b in x['bones']:assert set(b)=={'translation','scale'}and len(b['translation'])==len(b['scale'])==3
validate(rows);index={(x['name'],x['mode']):x['bones']for x in rows}
edges=['source-early-unit','early-unit-current','source-current']
summary={domain:{edge:dict(translation=0,scale=0,first_translation=None,first_scale=None)for edge in edges}for domain in ['cc','sc','cs','ss']}
table=[]
for name in sorted(expected):
    domain=name.split(':')[1]
    for edge in edges:
        left,right={'source-early-unit':('source','early-unit'),'early-unit-current':('early-unit','current'),'source-current':('source','current')}[edge]
        values={k:0 for k in ['translation','scale']}
        for bone,a,b in zip(names,index[name,left],index[name,right],strict=True):
            for channel in values:
                if a[channel]!=b[channel]:
                    values[channel]+=1;summary[domain][edge][channel]+=1
                    first='first_'+channel
                    if summary[domain][edge][first]is None:summary[domain][edge][first]=dict(name=name,bone=bone,left=a[channel],right=b[channel])
        table.append(dict(name=name,edge=edge,**values))
assert {k:summary[k]['source-current']['translation']for k in summary}==dict(cc=0,sc=2564,cs=48,ss=403)
assert {k:summary[k]['source-current']['scale']for k in summary}==dict(cc=0,sc=1487,cs=0,ss=0)
assert sum(x['source-early-unit']['translation']for x in summary.values())>0,'Missing early unit stage signal'
assert sum(x['early-unit-current']['translation']+x['early-unit-current']['scale']for x in summary.values())>0,'Missing child projection signal'
negative=[]
for kind in ['missing-mode','duplicate-mode','wrong-mode','missing-bone']:
    if kind=='missing-mode':bad=rows[:-1]
    elif kind=='duplicate-mode':bad=[*rows[:-1],rows[0]]
    elif kind=='wrong-mode':bad=[dict(rows[0],mode='invalid'),*rows[1:]]
    else:bad=[dict(rows[0],bones=rows[0]['bones'][:-1]),*rows[1:]]
    try:validate(bad)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid T/S stage matrix accepted: '+kind)
result=dict(gate='PASS_NATIVE_CURRENT_HEAD_POSITION_SCALE_ENDPOINTS_AND_PROJECTION_STAGES',native_and_current_rows_each_exact=100488,mode_count=len(modes),domain_summaries=summary,negative_fixtures_rejected=negative,full_root_or_als_equivalence_claimed=False,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/('analysis-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('frame-table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps({d:{edge:{k:v[k]for k in ['translation','scale']}for edge,v in edges.items()}for d,edges in summary.items()}),flush=True)
