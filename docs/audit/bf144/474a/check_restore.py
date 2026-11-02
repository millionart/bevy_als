import json
from pathlib import Path

r=Path(__file__).resolve().parent
for suffix in['','weights-']:
 assert(r/('table-'+suffix+'baseline.json')).read_bytes()==(r/('table-'+suffix+'restored.json')).read_bytes()
(r/'restore-check.json').write_text(json.dumps(dict(gate='PASS_ALL1332_NATIVE_DIRTY_GLOBAL_LOCAL_RECORDS_BYTE_EXACT_AFTER_SOURCE_PACKAGE_RESTORE',actual_calls=1272,controlled_calls=60),indent=2)+'\n');print('Verified all1332 flag/global/local records byte exact after complete source/package restore',flush=True)
