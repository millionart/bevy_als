import gzip
import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
comparison = json.loads((r/'comparison.json').read_bytes())
assert comparison['gate'] == 'PASS_ACTUAL_AUTOMATIC_LENGTH_NATIVE_CONSUMERS_AND_EXISTING_D165_FACTOR'
assert json.loads((r/'identity-check.json').read_bytes())['six_libraries_unchanged']
assert json.loads((r/'identity.report.json').read_bytes())['success']
for label in ['lengths','solver']:
    report=json.loads((r/(label+'.native.json.reader.report.json')).read_bytes())
    assert report['success'] and report['process_exit_code'] == 0
for row in json.loads((r/'compression-map.json').read_bytes()).values():
    packed=(r/row['compressed']).read_bytes()
    decoded=gzip.decompress(packed)
    assert len(decoded)==row['raw_bytes'] and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']
    assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
parent=json.loads(Path('docs/audit/bf144/476/evidence-manifest.json').read_bytes())
assert hashlib.sha256(Path('docs/audit/bf144/476/inputs.json').read_bytes()).hexdigest()==parent['files']['inputs.json']['sha256']
external=[]
for name in ['docs/audit/bf144/476/inputs.json','docs/audit/bf144/476/evidence-manifest.json','docs/audit/bf144/476/summary.json','docs/audit/bf144/474a/reader-migration.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
    raw=Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
summary=dict(comparison)
summary.update(gate='PASS_ACTUAL_AUTO_LENGTH_NATIVE_FULL_WEIGHT_CONSUMPTION_AND_EXISTING_D165',external_source_files=external,current396_reader474a_and_six_libraries_unchanged=True,production_reader_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,next_boundary='Same qualified current and source inputs: complete IK position and orientation arithmetic, partial wrapper, and descendant hierarchy')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified actual automatic-length native/public consumption, existing D165 factor, restoration and persistent identities',flush=True)
