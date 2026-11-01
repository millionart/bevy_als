import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '446':('PASS_ACTUAL_COLD_START_STATE_AND_INPUT_HISTORY_WITH_NATIVE_PREREQUISITES',67),
    '446b':('PASS_BOUNDED318_NATIVE_HISTORY_AND_OPTIONAL_INITIAL_POSE_WITH_LEGACY_COMPATIBILITY',67),
    '446c':('PASS_COMPLETE318_EXTERNAL_INPUT_AND_PRODUCER_BOUNDARY_AUDIT',42),
    '446d':('PASS_OPTIONAL_INTERNAL_PROXY_CURVES_AND_NATIVE_FRAME_READER_WITH_LEGACY_COMPATIBILITY',65),
    '446e':('PASS_TARGET_ACTUAL_PRIOR_CURVE_INPUTS_AND_DISTINCT_FRAME_BOUNDARY',63),
    '446f':('PASS_MATCHED_NATIVE_ENGINE_FRAME_CONTROL_AND_BOOT_CANDIDATE_REJECTION',62),
    '446g':('PASS_COLD319_ACTUAL_FORCE_PHYSICS_AND_D31_FIRST_CONSUMER',91),
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
    assert summary['screenshots']==0 and summary['performance_acceptance']=='paused'and summary['source_bytes_restored']
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
    if kind=='missing':del bad['446f']
    else:bad['446f']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
result=dict(gate='PASS_INITIALIZATION_AND_FULL_EXTERNAL_HISTORY_AUDIT_WITH_KNOWN_DIFFERENCES',children=children,verified_git_evidence_files=sum(v['files']for v in children.values()),negative_child_scope_and_hash_cases_rejected=3,
    target_actual_query_history=319,matched_external_positive_inputs=318,source_actual_controller_inputs_verified=318,
    boot_curve_publication_candidate_rejected_by_matching_callback_frame=True,default_pose_eligibility_matches_target_all318=True,
    actual_force_and_raw_physics_rows_exact=319,cold_spring_rows_exact=79,first_cold_position_known_difference='BF144-D31',other_known_consumers=['BF144-D149','BF144-D201'],
    new_differences=[],total_confirmed_differences=271,priority_counts=dict(P2=116,P3=155),production_or_source_assets_fixed=False,
    source_target_all_internal_inputs_matched=False,all_trajectory_clock_or_pose_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Recompute complete frame-matched source318 versus target319 state, player clocks and Root pose input residuals using the controlled source history',
    screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
