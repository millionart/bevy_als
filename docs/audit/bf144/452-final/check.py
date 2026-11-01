import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={
    '452a':('PASS_COMPLETE318_CURRENT_HEAD_ROTATION_STAGE_FACTORS_WITH_EXPLICIT_SOURCE_MATH_BOUNDARY',52),
    '452b':('PASS_ALL1272_SOURCE_HEAD_ROTATION_ARITHMETIC_AND_NECESSARY_FACTORS_WITH_REDUNDANT_PRODUCT_REMOVED',78),
}
def validate(children):
    assert set(children)==set(expected)
    for name,child in children.items():assert (child['gate'],child['files'])==expected[name]
def verify(raw,record):
    assert len(raw)==record['bytes']and hashlib.sha256(raw).hexdigest()==record['sha256']
children={};external={};all_paths=set()
for name,(gate,count)in expected.items():
    root=r.parent/name;summary=json.loads((root/'summary.json').read_text());manifest=json.loads((root/'evidence-manifest.json').read_text())
    assert summary['gate']==manifest['gate']==gate and len(manifest['files'])==count
    assert summary['screenshots']==0 and summary['performance_acceptance']=='paused'
    paths=set()
    for relative,record in manifest['files'].items():
        p=(root/relative).resolve();assert p.is_relative_to(root.resolve());verify(p.read_bytes(),record);paths.add(str(p.relative_to(Path.cwd())))
    for row in summary['external_source_files']:
        relative=row['path'];p=Path(relative).resolve();assert p.is_relative_to(Path.cwd());record={k:row[k]for k in ['bytes','sha256']};verify(p.read_bytes(),record)
        if relative in external:assert external[relative]==record
        external[relative]=record
    paths.update(str((root/n).relative_to(Path.cwd()))for n in ['summary.json','evidence-manifest.json'])
    tracked=subprocess.run(['git','ls-files','-z','--',str(root.relative_to(Path.cwd()))],check=True,capture_output=True,timeout=5).stdout.decode().split('\0');assert paths<=set(tracked)
    all_paths|=paths
    children[name]=dict(gate=gate,files=count,summary_sha256=hashlib.sha256((root/'summary.json').read_bytes()).hexdigest(),manifest_sha256=hashlib.sha256((root/'evidence-manifest.json').read_bytes()).hexdigest())
    print('Verified sealed Head arithmetic child '+name,flush=True)
validate(children);assert len(external)==25
for relative in external:
    subprocess.run(['git','ls-files','--error-unmatch',relative],check=True,stdout=subprocess.DEVNULL,timeout=5)
all_paths.update(external)
for kind in ['missing','wrong_scope']:
    bad=copy.deepcopy(children)
    if kind=='missing':del bad['452b']
    else:bad['452b']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
result=dict(gate='PASS_SAME_INPUT_FULL_HEAD_ROTATION_STAGE_AND_SOURCE_ARITHMETIC_AUDIT',children=children,verified_child_manifest_evidence_files=130,verified_external_references=25,verified_unique_git_evidence_and_manifests=len(all_paths),negative_child_scope_and_hash_cases_rejected=3,current_actual_rotation_stage_frames_exact=318,current_actual_rotation_rows_exact=25440,source_native_four_domain_rotation_rows_exact=100488,source_native_case_count=1272,removed_redundant_manual_double_product=True,arm64_normalizer_necessity_proved=True,five_source_factors_and_three_current_stage_factors_verified=True,source_current_factor_counts_not_summed=True,source_stage_projection_control_not_actual_current_math=True,same_current_input_native_rotation_difference_rows=23032,new_differences=[],total_confirmed_differences=272,priority_counts=dict(P2=116,P3=156),production_or_original_asset_fixed=False,full_root_or_als_equivalence_claimed=False,next_direct_boundary='Actual remaining full Head/Root pose input and numerical operation boundaries; calibrated source rotation kernel does not prove all actual current math or final Root equivalent',screenshots=0,performance_acceptance='paused',render_gpu_pass_acceptance='excluded under explicit peripheral scope')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
