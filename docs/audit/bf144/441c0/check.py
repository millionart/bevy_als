import hashlib
import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
for name in ['before','no-root','overwrite','absolute','absolute-report']:
    row=read(name+'.report.json');assert not row['success'] and row['process_exit_code']==1,name
for name in ['baseline','restored']:
    assert read(name+'.report.json')['success'],name
    assert read(name+'.json')['game_started']is False
assert read('restored.json')['environment_and_report_paths_relative']
p=Path('tools/run_animation_visual_validation.ps1');assert p.read_bytes()==(r/'runner.final.ps1').read_bytes()
for name in ['baseline','restored']:
    assert(r/'fixtures'/name/'existing/sentinel.txt').read_text()=='keep original evidence'
report={'gate':'PASS_GIT_VISUAL_EVIDENCE_ROUTING_AND_FOUR_PROTECTION_ABLATIONS','original_target_only_path_rejects_git_directory':True,'git_path_accepted':True,'target_path_rejected':True,'existing_evidence_not_deleted':True,'environment_and_runner_paths_relative':True,'root_overwrite_environment_and_report_guards_have_failed_counterexamples':True,'final_source_bytes_restored_and_fresh_powershell_gate_passed':True,'runner_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'test_sha256':hashlib.sha256(Path('tools/tests/test_visual_evidence_routing.ps1').read_bytes()).hexdigest(),'game_started':False,'screenshots':0,'performance_acceptance':'paused'}
(r/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
