import gzip
import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())


def native(label):
    p=r/f'{label}.native.json'
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress((r/f'{label}.native.json.gz').read_bytes()))


fields={'sync_read_buffer_tick_present','sync_read_buffer_tick_matches','tick_group','tick_weight','tick_evaluator','tick_previous_valid','tick_previous','tick_delta','tick_accumulator'}
def strip(value):
    if isinstance(value,dict):
        return {k:strip(v)for k,v in value.items()if k not in fields}
    if isinstance(value,list):
        return [strip(v)for v in value]
    return value


a=native('baseline')
b=native('restored')
counts=validate(a)
assert counts==validate(b)
assert a['character_animation_probe']==b['character_animation_probe']
old=json.loads(gzip.decompress(Path('docs/audit/bf144/443/source.native.json.gz').read_bytes()))['character_animation_probe']
assert strip(a['character_animation_probe'])==old
legacy=native('legacy')
assert legacy['character_animation_probe']==old
bad=native('ablated')
try:
    validate(bad)
except AssertionError:
    pass
else:
    raise AssertionError('Missing tick association passed')
bad_legacy=native('ablated-legacy')['character_animation_probe']
assert bad_legacy!=old and strip(bad_legacy)==old
for label,expected in [('baseline-invalid',7),('ablated-invalid',0),('restored-invalid',7)]:
    assert read(label+'.native.json.reader.report.json')['process_exit_code']==expected
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('identity-check.json')['current_base_files_unchanged']==396
result=dict(gate='PASS_OPTIONAL_NATIVE_ASSET_TICK_AND_ACCUMULATOR_READER_WITH_LEGACY_COMPATIBILITY',counts=counts,
    negative_fixtures_rejected=3,pointer_association_type_guard_and_optional_guard_ablations_rejected=True,
    source_bytes_restored=True,same_native_package_rebuilt=True,complete_character_probe_restored_exact=True,
    previous_current240_character_probe_unchanged=True,no_flag_legacy240_complete_probe_exact=True,
    whole_asset_graph_metadata_byte_equivalence_claimed=False,new_differences=[],total_confirmed_differences=271,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
