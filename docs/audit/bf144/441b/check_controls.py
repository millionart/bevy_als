import hashlib
import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
state=read('ablation.json')
assert(r/'curve_excerpt.rs').read_bytes()==(r/'curve_excerpt.final.rs').read_bytes()
assert(r/'source_model.py').read_bytes()==(r/'source_model.final.py').read_bytes()
assert hashlib.sha256((r/'source-parameters.json').read_bytes()).hexdigest()==state['baseline_source_parameters_sha256']
assert(r/'restored.f32le').read_bytes()==(r/'excerpt.f32le').read_bytes()
for n in ['restored-build.report.json','restored.report.json','check_source.report.json','check_scalars.report.json']:
    assert read(n)['success'],n
assert not read('source-linear-ablation.report.json')['success']
from source_model import f
curves=read('curves.native.json')['blend_profile_probe']['samples']
code=(r/'source_model.final.py').read_text();original='return lerp(lerp(x,y,alpha),lerp(y,z,alpha),alpha)';assert code.count(original)==1
negative=code.replace(original,'return lerp(p0,p3,alpha)');(r/'source_model.linear_ablation.py').write_text(negative)
namespace={};exec(compile(negative,'source_linear_ablation','exec'),namespace)
failures=[{'axis':axis,'time':sample['time'],'expected':sample['value'],'actual':namespace['rich'](row['custom_curve_keys'],sample['time'])}for axis,row in enumerate(curves)for sample in row['custom_curve_time_samples']if namespace['rich'](row['custom_curve_keys'],sample['time'])!=sample['value']]
assert failures and failures[0]['axis']==0 and failures[0]['time']==2.125
report={'gate':'PASS_SOURCE_CUBIC_AND_TARGET_KEY_CONTROL_ABLATIONS_SAME_TARGET_RESTORATION','source_cubic_removal_fails_native_gait2_125':True,'target_key_control_removal_loses_two_scalar_signals':True,'source_bytes_restored':True,'same_target_excerpt_recompiled':True,'full_scalar_bytes_and_source240_report_restored_exact':True,'target_full_runtime_equivalence_claimed':False,'source_negative_first_failure':failures[0],'source_negative_failure_count':len(failures)}
(r/'controls-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
