import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
for name in ['source-producer','angle-stages','actual-consumers','restored','no-normalization','no-angle-stage']:
    assert read(name+'.report.json')['success'],name
assert read('native.json.reader.report.json')['success']and read('actual.native.json.reader.report.json')['success']
assert read('check.report.json')['success']and read('identity.report.json')['success']
assert not read('consumers-build.report.json')['success']and not read('combined-build.report.json')['success']
v=read('summary.json');assert v['confirmed_active_differences']==sum(l.startswith('- [ ] BF144-D')for l in Path('CHECKLIST.md').read_text().splitlines())==268
print(json.dumps({'gate':v['gate'],'new_differences':v['new_differences'],'confirmed':268,'parent442_still_unverified_fallback':True}),flush=True)
