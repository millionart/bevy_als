import importlib.util
import json
from pathlib import Path
r=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('source_model',Path('docs/audit/bf144/441b/source_model.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
read=lambda n:json.loads((r/n).read_text())
rows=read('runtime-assets-root/parameters.json')['rows'];native=read('curves-at-runtime.native.json')['blend_profile_probe']['samples'];values=[{x['time']:x['value']for x in row['custom_curve_time_samples']}for row in native]
keys=[]
for axis,row in enumerate(native):
    mapped=[]
    for k in row['custom_curve_keys']:
        k=dict(k)
        for name in ['value','arrive_tangent','leave_tangent']:k[name]=m.f(k[name]/(100 if axis<2 else 1))
        mapped.append(k)
    keys.append(mapped)
for row in rows:
    amount=row['gait_amount'];row['source_curve_cm']=[v[amount]for v in values];row['source_order_si']=[m.rich(k,amount)for k in keys]
assert len(rows)==240 and all(x['locomotion']=='Grounded'and x['stance']=='Standing'and x['rotation_mode']=='ViewDirection'and x['allowed_gait']=='Running'for x in rows)
(r/'kernel-inputs.json').write_text(json.dumps(rows,separators=(',',':'))+'\n');print('Prepared240 actual consumed-vector parameter cases with precise native same-gait curve results',flush=True)
