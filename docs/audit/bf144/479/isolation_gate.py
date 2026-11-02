import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
case=sys.argv[1]
p=r/('replay-unit-'+case+'.json')
result=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
assert result['source_exact']==result['public_exact']==5192 and result['current_actual']==1272
assert len(result['outputs'])==5192*7
source={row['row']:row['result']for row in result['outputs']if row['mask']==31}
changed=[]
for row in result['outputs']:
    if row['mask']in [31,0]:
        continue
    before=source[row['row']]
    after=row['result']
    if before['joint']!=after['joint']or before['end']!=after['end']:
        changed.append(dict(row=row['row'],mask=row['mask'],position_mask=row['position_mask'],before=[before['joint'],before['end']],after=[after['joint'],after['end']]))
report=dict(gate='ORIENTATION_ONLY_FACTOR_POSITION_ISOLATION',case=case,source_and_public_complete_endpoints_still_exact=True,changed_position_records=len(changed),first=changed[:1])
(r/('isolation-'+case+'.json')).write_text(json.dumps(report,indent=2)+'\n')
assert not changed,'Orientation-only controls must preserve independently qualified source positions'
print('Qualified isolated orientation controls with all positions fixed across5192 cases',flush=True)
