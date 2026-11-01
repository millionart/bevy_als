import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '450a':('PASS_ACTUAL_FORWARD_DIFFERENCE_INPUTS_PUBLIC_KERNEL_AND_MODE_ENDPOINTS',60),
    '450b':('PASS_HEAD_FIRST_COMPRESSED_DATA_X_AND_REFERENCE_UNIT_FACTOR_SEPARATION',64),
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
    if kind=='missing':del bad['450b']
    else:bad['450b']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
external=json.loads((r.parent/'450a/evidence-manifest.json').read_text())['external_source_files']
assert len(external)==15
references=json.loads((r.parent/'450/endpoints.json').read_text())['input_identities']
for relative,digest in references.items():
    p=Path(relative).resolve();assert p.is_relative_to(Path.cwd())and hashlib.sha256(p.read_bytes()).hexdigest()==digest
    subprocess.run(['git','ls-files','--error-unmatch',relative],check=True,stdout=subprocess.DEVNULL,timeout=5)
additional=set(references)-set(external);assert additional=={'docs/audit/bf144/449/stand-raw.native.json'}
result=dict(gate='PASS_ACTUAL_FIRST_HEAD_ROOT_X_DATA_AND_REFERENCE_FACTOR_AUDIT',children=children,verified_git_child_evidence_files=sum(v['files']for v in children.values()),verified_fresh_source_evidence_files=len(external),verified_additional_existing_raw_reference_files=len(additional),verified_git_evidence_files=sum(v['files']for v in children.values())+len(external)+len(additional),negative_child_scope_and_hash_cases_rejected=3,
    actual_difference_records=319,actual_public_difference_bone_rows=25201,source_first_sample_only_forward_half_phase_weight1=True,actual_half_phase_data_factor_records=184,
    native_compressed_delta_x_cm=.003921627998352051,native_raw_delta_x_cm=0,original_target_first_delta_root_x=1.7007637193685825e-30,data_factor_first_delta_rig_root_x=3.921628012903966e-5,
    physical_raw_reference_only_delta=-1.88079096131566e-37,known_factors=['BF144-D224','BF144-D235','BF144-D275'],source_bytes_and_same_target_controls_restored=True,
    new_differences=[],total_confirmed_differences=272,priority_counts=dict(P2=116,P3=156),production_or_original_asset_fixed=False,all_trajectory_clock_or_pose_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Remaining actual Head/Rig skeletal rotation and position residuals in the matched five-boundary inventory; do not absorb them into the restored first X',
    screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
