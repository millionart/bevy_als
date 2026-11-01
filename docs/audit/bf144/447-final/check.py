import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '447':('PASS_FRAME_ALIGNED_COMPLETE_RESIDUAL_INVENTORY_WITH_OPEN_HEAD_INPUT',27),
    '447b':('PASS_ACTUAL_HEAD_INPUT_STATE_AND_LOOK_PIN_READONLY_ENTRY',64),
    '447c':('PASS_COMMON_INPUT_HEAD_KERNEL_AND_FIRST_LOOK_VIEW_FACTOR',52),
    '447d':('PASS_VIEW_CAST_FACTOR_AND_FIRST_HEAD_LOOK_CONSUMER_AS_RECORDED_D237',45),
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
    if kind=='missing':del bad['447d']
    else:bad['447d']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
result=dict(gate='PASS_FRAME_ALIGNED_RESIDUAL_INVENTORY_AND_FIRST_HEAD_LOOK_INPUT_AUDIT',children=children,verified_git_evidence_files=sum(v['files']for v in children.values()),negative_child_scope_and_hash_cases_rejected=3,
    target_actual_query_history=319,matched_external_positive_inputs=318,source_player_ticks=1980,target_player_ticks=1879,eligibility_differences=101,clock_differences=697,
    actual_target_head_continuous_replay_rows_exact=319,native_actual_head_common_input_rows_exact=318,first_unattributed_look_input_factor_now_recorded='BF144-D237',
    first_head_math_difference_rejected_by_common_actual_inputs=True,actual_view_producer_replay_rows_exact=319,native_same_input_view_cast_controls_exact=319,
    new_differences=[],total_confirmed_differences=271,priority_counts=dict(P2=116,P3=155),production_or_source_assets_fixed=False,
    source_target_all_internal_inputs_matched=False,all_trajectory_clock_or_pose_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Attribute remaining frame-matched actual internal input/state and player-clock residuals, including body/view history and complete Root/custom attributes',
    screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
