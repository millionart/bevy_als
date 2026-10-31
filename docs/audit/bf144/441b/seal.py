import gzip
import hashlib
import json
import re
import subprocess
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
for n in ['settings-warm.native.json.reader.report.json','curves.native.json.reader.report.json','character.native.json.reader.report.json','linear.native.json.reader.report.json','restored-build.report.json','restored.report.json','check_source.report.json','check_scalars.report.json','check_controls.report.json','check_identity.report.json']:
    assert read(n)['success'],n
assert read('settings.native.json.reader.report.json')['exit_reason']=='wall_timeout'
assert not read('source-linear-ablation.report.json')['success']
assert all(v=='PASS'for v in read('watchdog.self-test.json').values())
assert read('identity-check.json')['current_files']==396
source=read('source-parameters.json');assert len(source['parameter_frames'])==240
summary={'gate':'PASS_RECOVERED_SOURCE240_NATIVE_PARAMETER_AND_SCALAR_FACTS','source_curve_samples':81,'new_source_parameter_frames':240,'new_current_identity_files':396,'D267_key_fit_revalidated':True,'D268_unit_stage_revalidated_native_controls':4,'source_and_target_excerpt_ablations_restored':True,'old12018_identity_chain':'missing; not revalidated','deleted_target240_and_rlib':'not restored;441c pending','evidence_directory':'docs/audit/bf144/441b','screenshots':0,'performance_acceptance':'paused'}
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
files={}
for p in sorted(r.iterdir()):
    if not p.is_file()or p.suffix not in ['.json','.gz','.py','.ps1','.rs','.txt','.f32le','.md']or p.name in ['evidence-manifest.json','seal.report.json']:continue
    b=p.read_bytes();plain=gzip.decompress(b)if p.suffix=='.gz'else b
    if p.suffix!='.f32le':
        text=plain.decode('utf-8');prefixes=['/'+name+'/'for name in ['Users','Volumes']];assert not any(prefix in text for prefix in prefixes)and not re.search(r'[A-Za-z]:\\',text),p.name
    files[p.name]={'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)}
    if p.suffix=='.gz':files[p.name]['uncompressed_sha256']=hashlib.sha256(plain).hexdigest()
    relative=str(p.relative_to(Path.cwd()));subprocess.run(['git','ls-files','--error-unmatch',relative],check=True,stdout=subprocess.DEVNULL,timeout=5)
manifest={'format':'tracked_audit_evidence_v1','gate':summary['gate'],'files':files,'build_directories_used_for_long_term_evidence':False,'previous_deleted_seal_not_reconstructed':True}
(r/'evidence-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({'gate':summary['gate'],'tracked_files_verified':len(files),'all_paths_relative_and_privacy_checked':True}),flush=True)
