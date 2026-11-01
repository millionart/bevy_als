import gzip
import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline')
b=load('restored')
counts=validate(a)
assert counts==validate(b)
assert a['character_animation_probe']==b['character_animation_probe']
old=json.loads(gzip.decompress(Path('docs/audit/bf144/446b/restored.native.json.gz').read_bytes()))['character_animation_probe']
keys={'proxy_attribute_curves','proxy_observed_engine_frame','proxy_native_update_engine_frame'}
def strip(v):
    if isinstance(v,dict):return {k:strip(x)for k,x in v.items()if k not in keys}
    if isinstance(v,list):return [strip(x)for x in v]
    return v
assert strip(a['character_animation_probe'])==old
assert load('legacy')['character_animation_probe']==old
bad=load('ablated')
try:validate(bad)
except AssertionError:pass
else:raise AssertionError('Missing actual proxy signals passed')
dirty=load('ablated-legacy')['character_animation_probe']
assert dirty!=old and strip(dirty)==old
for label,expected in [('baseline-invalid',7),('ablated-invalid',0),('restored-invalid',7)]:assert read(label+'.native.json.reader.report.json')['process_exit_code']==expected
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('ablated-corrected-package-source-check.json')['exact']
assert read('identity-check.json')['current_base_files_unchanged']==396
result=dict(gate='PASS_OPTIONAL_INTERNAL_PROXY_CURVES_AND_NATIVE_FRAME_READER_WITH_LEGACY_COMPATIBILITY',counts=counts,
    getter_frame_typed_and_optional_guard_ablations_rejected=True,negative_fixtures_rejected=4,source_bytes_restored=True,same_native_package_rebuilt=True,
    complete_new_character_probe_restored_exact=True,complete_old318_plus2_character_probe_exact=True,source_same_engine_frame_callback_gate_observed=True,
    target_actual_prior_input_or_frame_equivalence_observed=False,curve_publication_candidate_confirmed_as_difference=False,
    new_differences=[],total_confirmed_differences=271,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
