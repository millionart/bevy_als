import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
source=json.loads(gzip.decompress(Path('docs/audit/bf144/446f/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
target=json.loads(Path('docs/audit/bf144/446g/runtime-restored/parameters.json').read_text())['animation_before']


def validate(case,rows):
    initial=next(v for v in case['initialization']['instances']if v['main'])
    start=initial['proxy_observed_engine_frame']
    app_start=rows[0]['curve_frame_input']['app_frame']
    for index,(frame,row)in enumerate(zip(case['frames'],rows[1:],strict=True)):
        before=next(v for v in frame['instances_before_tick']if v['main'])
        after=next(v for v in frame['linked_instances']if v['main'])
        assert before['proxy_observed_engine_frame']==start+index+1
        assert before['proxy_native_update_engine_frame']<before['proxy_observed_engine_frame']
        assert after['proxy_native_update_engine_frame']==before['proxy_observed_engine_frame']
        assert row['curve_frame_input']['app_frame']==app_start+index+1
    assert len(case['frames'])==318


validate(source,target)
old=json.loads(gzip.decompress(Path('docs/audit/bf144/446d/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
try:validate(old,target)
except AssertionError:pass
else:raise AssertionError('Same-engine-frame source passed the matching callback-frame contract')
try:validate(source,target[1:]+target[:1])
except AssertionError:pass
else:raise AssertionError('Shifted target application frame passed')
result=dict(gate='PASS_NATIVE_AND_TARGET_FIRST_ORDINARY_NEW_FRAME_QUALIFICATION',positive_rows=318,old_same_frame_rejected=True,shifted_target_rejected=True)
(r/'phase-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
