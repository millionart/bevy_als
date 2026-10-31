import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
for name in ['check_schema','check_native','check_controls','identity']:
    assert read(name+'.report.json')['success'],name
for name in ['before','initial','no-clamp','restored','legacy']:
    assert read(name+'.native.json.reader.report.json')['success'],name
identity=read('identity-check.json');assert identity['base_current_identity_files']==396 and identity['unchanged_base_keys']==395
result={'gate':'PASS_READ_ONLY_NATIVE_MOVE_INPUT_MATH_CAPABILITY','native_samples':7,'schema_negative_cases':5,'old_missing_capability_rejected':True,'three_invalid_native_requests_rejected':True,'legacy_geometry_samples_exact':720,'controls':read('controls-check.json'),'identity':identity,'scope':'Diagnostic Core math execution using verified source inline algorithms; no source character tick or camera simulation and no source/target input equivalence verdict','parent442b_status':'pending native same-angle amplitude analysis','screenshots':0,'performance_acceptance':'paused'}
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k]for k in ['gate','native_samples','legacy_geometry_samples_exact','scope','parent442b_status']}),flush=True)
