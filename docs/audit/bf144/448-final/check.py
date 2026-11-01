import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '448a':('PASS_NATIVE318_FIVE_ROOT_POSE_BOUNDARIES_WITH_CURRENT_ATTRIBUTE_AND_LEGACY_CONTROLS',39),
    '448b':('PASS_ACTUAL319_FIVE_POSE_BOUNDARIES_AND_ROOT_OUTPUT_READONLY_ENTRY',64),
    '448c':('PASS_FIVE_POSE_RESIDUAL_INVENTORY_WITH_ACTUAL_SCENE_TARGET_PREREQUISITE',25),
    '448d':('PASS_ACTUAL_SCENE_TARGET_QUALIFICATION_AND_EXTRA_SCALE_CANDIDATE_REJECTION',60),
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
    paths.update(str((root/n).relative_to(Path.cwd()))for n in ['summary.json','evidence-manifest.json'])
    tracked=subprocess.run(['git','ls-files','-z','--',str(root.relative_to(Path.cwd()))],check=True,capture_output=True,timeout=5).stdout.decode().split('\0')
    assert paths<=set(tracked)
    children[name]=dict(gate=gate,files=count,summary_sha256=hashlib.sha256((root/'summary.json').read_bytes()).hexdigest(),manifest_sha256=hashlib.sha256((root/'evidence-manifest.json').read_bytes()).hexdigest())
    print('Verified sealed child '+name,flush=True)
validate(children)
for kind in ['missing','wrong_scope']:
    bad=copy.deepcopy(children)
    if kind=='missing':del bad['448d']
    else:bad['448d']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
result=dict(gate='PASS_ACTUAL_ROOT_BOUNDARY_AND_CURRENT_ATTRIBUTE_AUDIT_WITH_OPEN_BONE_RESIDUALS',children=children,verified_git_evidence_files=sum(v['files']for v in children.values()),negative_child_scope_and_hash_cases_rejected=3,
    native_actual_positive_frames=318,target_actual_queries_per_phase=319,root_boundary_pairs=1590,source_target_bone_pairs=125610,
    native_five_current_boundaries_custom_attributes_empty=True,target_attribute_absence_only_type_fact=True,source_raw_root_and_mesh_finalize_stages_separate=True,
    default_target_root_equals_root_cache=True,optional_bone_channels_preserved=True,extra_scene_root_nonidentity_pose_rows=44,extra_scene_root_actual_target_count_zero_all319=True,
    extra_scene_root_scale_shrink_candidate_rejected_for_current_apply_chain=True,new_differences=[],total_confirmed_differences=271,priority_counts=dict(P2=116,P3=155),production_or_source_assets_fixed=False,
    all_trajectory_clock_or_pose_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Attribute first actual skeletal Root residual through BaseProviderInput pelvis Y, then actual Head additive and Rig inputs using fresh same-leaf source contracts',
    screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
