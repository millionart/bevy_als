import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
rows=read('mapping.native.json')['geometry_query_probe']['samples'];inputs=read('mapping-inputs.json');assert len(rows)==len(inputs)*3==720
length=[];units=[]
for i,x in enumerate(inputs):
    cm,projected,direct=rows[3*i:3*i+3]
    assert cm['scaled_distance_float']==x['cm_float_speed_model']
    assert direct['float_range_mapped']==x['actual_target_gait']
    for row in [cm,projected,direct]:
        m=row['input']['float_range_map'];assert m['input_range'][0]<=m['input']<=m['input_range'][1]
    value={'ordinal':i,'cm_float_speed':x['cm_float_speed_model'],'projected_meter_float_speed':x['projected_meter_float_speed'],'direct_meter_float_speed':x['direct_meter_float_speed'],'native_cm_map':cm['float_range_mapped'],'native_projected_meter_map':projected['float_range_mapped'],'native_direct_meter_map':direct['float_range_mapped']}
    if projected['float_range_mapped']!=direct['float_range_mapped']:length.append(value)
    if cm['float_range_mapped']!=projected['float_range_mapped']:units.append(value)
report={'gate':'PASS_NATIVE240_LENGTH_AND720_GAIT_MAPPING_PHASE_FACTORS','native_lengths_exact_to_source_double_then_cm_float_model':240,'native_direct_meter_maps_exact_to_current_public_gait':240,'all_unclamped_inputs_inside_selected_ranges_so_source_clamp_is_identity':True,'D256_length_phase_gait_rows':len(length),'D268_same_cm_float_speed_unit_mapping_rows':len(units),'net_common_cm_vs_direct_target_gait_rows':sum(rows[3*i]['float_range_mapped']!=rows[3*i+2]['float_range_mapped']for i in range(240)),'length_examples':length[:2],'mapping_examples':units[:2],'remaining':'Gait-state scalar consumers qualified; no claim every scalar changes a curve or physical motion'}
(r/'mapping-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
