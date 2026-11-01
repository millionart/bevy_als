import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/446e/observer.rs').read_text().replace('C446E','C447B').replace('c446e_','c447b_')
needle='"curve_frame_input":curve_frame_input,'
assert s.count(needle)==1
s=s.replace(needle,needle+'\n        "head_values":c447b_head_values(player,&graphs),')
s+=(r/'head_observer.rs').read_text()
(r/'observer.rs').write_text(s)
old=Path('docs/audit/bf144/446e/playground_observed.rs').read_text()
needle='    if let AppExit::Error(code) = app.run() {'
b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b)
original=Path('src/als_extras/levels/manual_playground.rs').read_text()
injection=old[a:b].replace('C446E','C447B').replace('c446e_','c447b_')
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/446e/rustc-args.json').read_text())
(r/'rustc-args.json').write_text(json.dumps([v.replace('/446e/','/447b/').replace('phase446e','phase447b')for v in args],indent=2)+'\n')
for name in ['run_python.ps1','compile.ps1','run.ps1','functional.ps1']:(r/name).write_text(Path('docs/audit/bf144/446e',name).read_text().replace('/446e','/447b').replace('ALS_C446E_OUTPUT','ALS_C447B_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/446e/observer.rs',added='Actual Head global/property/frame inputs, before/after state and Look binding cache',production_changed=False,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared actual Head parameter/state and Look pin observation',flush=True)
