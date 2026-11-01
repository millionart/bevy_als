import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '451a':('PASS_ACTUAL_FULL_HEAD_APPLY_ALPHA_SKELETON_AND_PUBLIC_KERNEL_ENTRY',66),
    '451b':('PASS_COMPLETE318_HEAD_FOUR_DOMAIN_MATRIX_WITH_EXPLICIT_MATH_BOUNDARY',66),
}


def validate(children):
    assert set(children)==set(expected)
    for name,child in children.items():assert (child['gate'],child['files'])==expected[name]


def verify(raw,record):
    assert len(raw)==record['bytes']and hashlib.sha256(raw).hexdigest()==record['sha256']


children={}
for name,(gate,count)in expected.items():
    root=r.parent/name
    summary=json.loads((root/'summary.json').read_text());manifest=json.loads((root/'evidence-manifest.json').read_text())
    assert summary['gate']==manifest['gate']==gate and len(manifest['files'])==count
    assert summary['screenshots']==0 and summary['performance_acceptance']=='paused'
    paths=set()
    for relative,record in manifest['files'].items():
        p=(root/relative).resolve();assert p.is_relative_to(root.resolve());verify(p.read_bytes(),record);paths.add(str(p.relative_to(Path.cwd())))
    for relative,record in manifest.get('external_source_files',{}).items():
        p=Path(relative).resolve();assert p.is_relative_to(Path.cwd());verify(p.read_bytes(),record)
        subprocess.run(['git','ls-files','--error-unmatch',relative],check=True,stdout=subprocess.DEVNULL,timeout=5)
    paths.update(str((root/n).relative_to(Path.cwd()))for n in ['summary.json','evidence-manifest.json'])
    tracked=subprocess.run(['git','ls-files','-z','--',str(root.relative_to(Path.cwd()))],check=True,capture_output=True,timeout=5).stdout.decode().split('\0')
    assert paths<=set(tracked)
    children[name]=dict(gate=gate,files=count,summary_sha256=hashlib.sha256((root/'summary.json').read_bytes()).hexdigest(),manifest_sha256=hashlib.sha256((root/'evidence-manifest.json').read_bytes()).hexdigest())
    print('Verified sealed child '+name,flush=True)
validate(children)
for kind in ['missing','wrong_scope']:
    bad=copy.deepcopy(children)
    if kind=='missing':del bad['451b']
    else:bad['451b']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
external=json.loads((r.parent/'451a/evidence-manifest.json').read_text())['external_source_files']
assert len(external)==8
references=json.loads((r.parent/'451b/input-identities.json').read_text())
for relative,digest in references.items():
    p=Path(relative).resolve();assert p.is_relative_to(Path.cwd())and hashlib.sha256(p.read_bytes()).hexdigest()==digest
    subprocess.run(['git','ls-files','--error-unmatch',relative],check=True,stdout=subprocess.DEVNULL,timeout=5)
additional={relative for relative in references if not relative.startswith('docs/audit/bf144/451a/')and relative not in external}
assert additional=={'docs/audit/bf144/448a/restored.native.json.gz'}
result=dict(gate='PASS_COMPLETE_HEAD_DATA_OPERATOR_MATRIX_AUDIT_WITH_OPEN_ROTATION_PIPELINE',children=children,verified_git_child_evidence_files=sum(v['files']for v in children.values()),verified_fresh_source_evidence_files=len(external),verified_existing_actual_source_corpus_files=len(additional),verified_git_evidence_files=sum(v['files']for v in children.values())+len(external)+len(additional),negative_child_scope_and_hash_cases_rejected=3,
    actual_head_public_replay_frames=319,actual_effective_alpha_all1=True,skeleton_snapshot_paths=82,namespace_anchor_defaults_none_preserved=True,
    matched_matrix_frames=318,domains=4,native_and_current_outputs_each=1272,bone_outputs_per_system=100488,native_ss_full_source_pose_and_curves_frames_exact=318,current_cc_full_actual_target_frames_exact=318,
    same_current_input_position_difference_rows=0,same_current_input_scale_difference_rows=0,same_current_input_rotation_difference_rows=23032,source_input_current_projection_kernel_position_difference_rows=403,source_input_current_projection_kernel_rotation_difference_rows=22719,
    data_and_operator_interactions_not_summed=True,new_differences=[],total_confirmed_differences=272,priority_counts=dict(P2=116,P3=156),production_or_original_asset_fixed=False,all_trajectory_clock_or_pose_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Actual same-current-input Head rotation pipeline: separate mesh accumulation/normalization stage and numerical precision across all matched frames, using the now-calibrated four-domain matrix',
    screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
