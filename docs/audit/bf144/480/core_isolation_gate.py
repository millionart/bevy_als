import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
case=sys.argv[1]
p=r/('replay-unit-'+case+'.json')
result=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
assert result['source_exact']==1332 and result['core_exact']==1330 and result['public_exact']==1302 and result['actual_exact']==1272
assert len(result['factors'])==144
full={(row['domain'],row['row'],row['weight']):row for row in result['factors']if row['mask']==7}
changed=[dict(domain=row['domain'],row=row['row'],weight=row['weight'],mask=row['mask'],before=full[(row['domain'],row['row'],row['weight'])]['result']['solved'],after=row['result']['solved'])for row in result['factors']if row['result']['solved']!=full[(row['domain'],row['row'],row['weight'])]['result']['solved']]
(r/('isolation-'+case+'.json')).write_text(json.dumps(dict(gate='PARTIAL_WRAPPER_SOURCE_SOLVER_INPUT_ISOLATION',case=case,complete_source_and_public_endpoints_preserved=True,changed_solver_records=len(changed),first=changed[:1]),indent=2)+'\n')
assert not changed,'Partial wrapper factors must hold the independently qualified solver result fixed'
print('Qualified144 isolated partial wrapper records with complete solver TQS inputs held fixed',flush=True)
