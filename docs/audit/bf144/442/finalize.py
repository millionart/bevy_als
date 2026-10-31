import hashlib
import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('replay.json');b=read('raw-control.json');assert len(a)==len(b)==240
assert all(x['actual']==x['computed']for x in a)
assert sum(x['actual']!=x['computed']for x in b)==80
assert(r/'replay.rs').read_bytes()==(r/'replay.final.rs').read_bytes()
assert(r/'replay.json').read_bytes()==(r/'restored.json').read_bytes()
for name in ['compile','check','facts','replay-build','replay','raw-control-build','raw-control','restored-build','restored']:
    assert read(name+'.report.json')['success'],name
assert read('runtime-baseline/process.report.json')['success']
report=read('selection-check.json');report['gate']='PASS_VIEW_SELECTION_PHASE_AND_ACTUAL240_DIRECTION_REPLAY';report['source_bytes_restored_and_same_target_recompiled']=True;report['full_replay_outputs_byte_exact_after_restore']=True;report['source_fact_units']=len(read('source-facts.json')['source_first_functions']);report['evidence_directory']='docs/audit/bf144/442';report['parent442_status']='incomplete; native same-angle amplitude and fallback boundaries remain';report['replay_source_sha256']=hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()
(r/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k]for k in ['gate','actual_direction_matches_published_view_formula','published_yaw_matches_previous_last_camera','raw_control_yaw_substitution_fails_moving_rows','parent442_status']}),flush=True)
