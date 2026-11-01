import json
from pathlib import Path
from check import load,read

r=Path(__file__).resolve().parent
legacy=load('legacy')['character_animation_probe'];old=load('old')['character_animation_probe'];assert legacy==old
current=load('default')['character_animation_probe'];previous=read(Path('docs/audit/bf144/446f/restored.native.json.gz'))['character_animation_probe'];assert current==previous
curves=read(r/'curves.native.json')['blend_profile_probe'];old_curves=read(Path('docs/audit/bf144/441b/curves.native.json'))['blend_profile_probe'];assert curves==old_curves
for label in ['legacy','default','curves']:
    report=read(r/(label+'.native.json.reader.report.json'));assert report['success']and report['process_exit_code']==0
for label in ['invalid','missing-boundaries']:
    report=read(r/(label+'.native.json.reader.report.json'));assert report['process_exit_code']==7
report=read(r/'no-parent-guard.native.json.reader.report.json');assert report['success']and report['process_exit_code']==0
result=dict(gate='PASS_OLD_FIVE_POSE_DEFAULT_CHARACTER_AND_FULL_CURVEVECTOR_COMPATIBILITY',old_five_boundary_character_probe_exact=True,old_default_character_probe_exact=True,curve_vector_three_axes_full_probe_exact=True,typed_and_missing_parent_invalid_requests_rejected=True,parent_guard_ablation_accepts_invalid_request=True,performance_acceptance='paused',screenshots=0)
(r/'compatibility.json').write_text(json.dumps(result,indent=2)+'\n');print('Verified complete old capture/default character and CurveVector probe compatibility with protocol counterexamples',flush=True)
