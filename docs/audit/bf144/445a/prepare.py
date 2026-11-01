import copy
import gzip
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
before=subprocess.check_output(['git','show','HEAD:'+reader])
(r/'reader-migration.json').write_text(json.dumps({reader:dict(before_sha256=hashlib.sha256(before).hexdigest(),after_sha256=hashlib.sha256(Path(reader).read_bytes()).hexdigest())},indent=2)+'\n')
base=json.loads(Path('docs/audit/bf144/443/source.request.json').read_text())
(r/'legacy.request.json').write_text(json.dumps(base,indent=2)+'\n')
base['capture_asset_player_ticks']=True
(r/'request.json').write_text(json.dumps(base,indent=2)+'\n')
bad=copy.deepcopy(base)
bad['capture_asset_player_ticks']='true'
(r/'invalid.request.json').write_text(json.dumps(bad,indent=2)+'\n')
old=json.loads(gzip.decompress(Path('docs/audit/bf144/443/source.native.json.gz').read_bytes()))['character_animation_probe']
rows=[p for c in old['cases'] for f in c['frames'] for i in f['linked_instances'] for p in i['asset_players']]
assert len(rows)==20400 and all('sync_read_buffer_tick_present'not in p for p in rows)
(r/'old-contract-failure.json').write_text(json.dumps(dict(gate='REJECTED_OLD_REPORT_MISSING_ACTUAL_TICK_QUALIFICATION',source_report='docs/audit/bf144/443/source.native.json.gz',missing_actual_tick_rows=len(rows),cached_weight_not_used_as_tick_oracle=True),indent=2)+'\n')
print('Prepared optional native tick requests and rejected the old missing-field report',flush=True)
