import json
import sys
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
sys.path.insert(0,str(Path('tools/tests').resolve()))
import test_movement_input_math_probe as schema
key='geometry_query_probe'
a=read('initial.native.json')[key];b=read('no-clamp.native.json')[key];assert len(a['samples'])==len(b['samples'])==7
failed=False
try:schema.validate({key:b})
except AssertionError:failed=True
assert failed
byname={x['input']['name']:x['move_input']for x in b['samples']};assert byname['over']['clamped_x']==2 and byname['over']['cmc_input']=={'x':0,'y':1,'z':0}
accepted=read('no-yaw-guard-invalid.json');accepted=[accepted]if isinstance(accepted,dict)else accepted;assert accepted==[{'name':'yaw-string','expected_exit':0,'actual_exit':0}]
for n in ['initial-invalid.json','restored-invalid.json']:
    rows=read(n);assert len(rows)==3 and all(x['actual_exit']==x['expected_exit']==7 for x in rows)
assert read('restored.native.json')[key]==a
legacy=json.loads(Path('docs/audit/bf144/441c/mapping.native.json').read_text())[key]
assert read('legacy.native.json')[key]==legacy and len(legacy['samples'])==720
source=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');package=Path('target/ue_graph_source_reader_package/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert source.read_bytes()==package.read_bytes()==Path('tools/tmp/bf144/442b0/reader.final.cpp').read_bytes()
for name in ['initial-build','no-clamp-fixed-build','no-yaw-guard-build','restored-build']:assert read(name+'.report.json')['success']
report={'gate':'PASS_NATIVE_MOVE_INPUT_CLAMP_AND_TYPE_ABLATIONS_SAME_PACKAGE_RESTORATION','upstream_clamp_removal_rejected_despite_cmc_endpoint_canceling_over_magnitude':True,'yaw_number_type_guard_removal_accepts_numeric_string':True,'three_invalid_requests_rejected_before_and_after':True,'restored_seven_complete_native_stage_outputs_exact':True,'legacy720_geometry_mapping_samples_entire_probe_exact':True,'source_bytes_restored_and_equal_package_after_same_package_rebuild':True,'source_algorithm_mirrored_from_verified_inline_bodies_using_public_core_apis':True,'not_actual_character_input_tick_or_camera_simulation':True,'screenshots':0,'performance_acceptance':'paused'}
(r/'controls-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
