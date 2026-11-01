import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
current=json.loads(Path('docs/audit/bf144/461/inputs.json').read_text());native=json.loads(Path('docs/audit/bf144/462/table-restored.json').read_text());lookup={(x['query'],x['pc']):x for x in native};assert len(lookup)==636
for row in current['rows']:
 row['native']=lookup[(row['query'],row['pc'])]
 assert row['source_arguments_raw'][8]==row['native']['source_offset_cm']
(r/'inputs.json').write_text(json.dumps(current,separators=(',',':'))+'\n')
files={}
for name in ['docs/audit/bf144/461/inputs.json','docs/audit/bf144/462/table-restored.json','docs/audit/bf144/462/summary.json']:
 raw=Path(name).read_bytes();files[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
(r/'input-identities.json').write_text(json.dumps(files,indent=2)+'\n')
print('Joined636 qualified native world rays and hits to actual target trace inputs',flush=True)
