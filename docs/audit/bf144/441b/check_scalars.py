import json
import struct
from pathlib import Path
from source_model import f
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
curves=read('curves.native.json')['blend_profile_probe']['samples'];times=[v['time']for v in curves[0]['custom_curve_time_samples']]
raw=(r/'excerpt.f32le').read_bytes();assert len(raw)==len(times)*16
values=[struct.unpack_from('<4f',raw,i*16)for i in range(len(times))]
data=[]
for t,v in zip(times,values):
    if v[0]!=v[3]:data.append({'gait':t,'current_acceleration_si':v[0],'raw_key_only_acceleration_si':v[3]})
assert len(data)==2 and [v['gait']for v in data]==[2.125,2.25]
method=(r/'method-excerpt.txt').read_text();source=Path('src/als/settings/params.rs').read_text();assert source.count(method)==1 and method in(r/'curve_excerpt.rs').read_text()
actual=read('linear.native.json')['blend_profile_probe']['samples'];request=read('linear.request.json')['samples'];assert len(actual)==len(request)==4
unit=[]
for i,t in enumerate([1.1131258010864258,1.7916673421859741]):
    cm=actual[2*i]['frames'][0];si=actual[2*i+1]['frames'][0];alpha=f(t-1)
    assert cm['alpha']==si['alpha']==cm['two_way_effective_alpha']==si['two_way_effective_alpha']==alpha
    native=next(v['value']for v in curves[1]['custom_curve_time_samples']if v['time']==t);assert cm['two_way_curve']==native
    target=values[times.index(t)][1];assert si['two_way_curve']==target!=f(native/100)
    unit.append({'gait':t,'same_native_lerp_alpha':alpha,'native_cm_lerp':native,'native_si_lerp':si['two_way_curve'],'cm_lerp_then_si_f32':f(native/100),'unchanged_target_hermite_si':target})
report={'gate':'PASS_RAW_KEY_FIT_DATA_AND_NATIVE_UNIT_STAGE_DISTINCTIONS','unchanged_target_method_compiled_as_standalone_excerpt':True,'full_target_runtime_or_deleted_rlib_equivalence_claimed':False,'data_only_samples':data,'unit_only_native_controls':unit,'screenshots':0,'performance_acceptance':'paused'}
(r/'scalar-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
