import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '449a':('PASS_ACTUAL_IDLE_LEAF_PUBLIC_SAMPLER_AND_FRESH_SOURCE_UNIT_ENDPOINTS',62),
    '449b':('PASS_SINGLE_HEIGHT_FLOAT_EXPORT_UNIT_FACTOR_AND_ACTUAL_ROOT_CONSUMERS',63),
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
    if kind=='missing':del bad['449b']
    else:bad['449b']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
external=json.loads((r.parent/'449a/evidence-manifest.json').read_text())['external_source_files']
assert len(external)==14
result=dict(gate='PASS_ACTUAL_FIRST_ROOT_HEIGHT_INPUT_AUDIT_WITH_RECORDED_FLOAT_UNIT_FACTOR',children=children,verified_git_child_evidence_files=sum(v['files']for v in children.values()),verified_fresh_source_evidence_files=len(external),verified_git_evidence_files=sum(v['files']for v in children.values())+len(external),negative_child_scope_and_hash_cases_rejected=3,
    actual_updated_idle_leaf_records=239,actual_public_sample_bone_rows=18881,baseprovider_first_positive_rows=98,source_raw_and_compressed_first_height_equal=True,
    source_physical_height=.9675060510635376,target_float_exported_height=.9675059914588928,only_idle_pelvis_y_replaced_in_diagnostic_clone=True,actual_first_root_y_recovered=True,
    route_factor_removed_returns_original_height=True,once_guard_removal_rejected=True,finite_423_classification_reopen_closed=True,new_differences=['BF144-D275'],total_confirmed_differences=272,priority_counts=dict(P2=116,P3=156),production_or_original_asset_fixed=False,
    all_trajectory_clock_or_pose_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Remaining actual five-boundary bone/curve residuals, including Head additive and Rig inputs; do not assign them all to D224 or D275',
    screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
