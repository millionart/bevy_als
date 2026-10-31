import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('kernel-speeds.json');unit=read('no-unit.json');key=read('no-key.json');order=read('no-order.json')
assert all(x['source_order_si']==x['source_order_cm']for x in unit['rows'])
assert sum(x['current']!=x['raw_key']for x in unit['data_controls'])==2
assert all(x['current']==x['raw_key']for x in key['data_controls'])
assert[x['ordinal']for x in key['rows']if x['source_order_si']!=x['source_order_cm']]==[63]
assert all(x['current']==x['source_order_si']for x in order['rows'])
assert all(x['source_order_cm']==y['source_order_cm']for x,y in zip(a['rows'],order['rows']))
assert(r/'kernel.rs').read_bytes()==(r/'kernel.final.rs').read_bytes()
assert(r/'restored.json').read_bytes()==(r/'kernel-speeds.json').read_bytes()
for name in ['no-unit-build','no-unit','no-key-build','no-key','no-order-build','no-order','restored-build','restored']:
    assert read(name+'.report.json')['success'],name
result={'gate':'PASS_THREE_PARAMETER_FACTOR_ABLATIONS_AND_SAME_TARGET_BYTE_RESTORATION','unit_stage_removal_loses_frame63_signal':True,'key_control_removal_loses_two_controls_and_preserves_unit_signal':True,'source_order_removal_loses_frame124_signal_and_preserves_native_endpoint':True,'final_source_bytes_restored':True,'same_target_recompiled':True,'complete240_outputs_and_controls_restored_byte_exact':True}
(r/'controls-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
