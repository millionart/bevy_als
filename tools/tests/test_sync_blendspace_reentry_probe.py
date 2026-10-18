import json
import struct
import sys
from pathlib import Path

request = json.loads(Path(__file__).with_name('fixtures').joinpath('sync_blendspace_reentry.request.json').read_text())
probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
frames = probe['samples'][0]['frames']
expected = request['expected_reentry']
f32 = lambda value: struct.unpack('f', struct.pack('f', value))[0]
index = expected['player']
before = frames[expected['before_frame']]['players'][index]
after = frames[expected['reentry_frame']]['players'][index]
old = max(before['samples'], key=lambda sample: sample['weight'])
new = max(after['samples'], key=lambda sample: sample['weight'])
assert len(frames) == 49 and old['sample_index'] != new['sample_index']
assert all(frame['leader'] is None and all(not p['ticked'] for p in frame['players']) for frame in frames[16:25])
assert before['position'] == f32(old['time'] / expected['sample_duration'])
assert new['previous_time'] == f32(before['position'] * expected['sample_duration'])
assert new['previous_time'] != next(sample['time'] for sample in before['samples'] if sample['sample_index'] == new['sample_index'])
assert after['position'] == f32(new['time'] / expected['sample_duration'])
assert frames[25]['leader'] == index and frames[25]['marker_start']['valid']
print('Native dynamic reentry rebuilds the new leader sample from the saved normalized player clock PASS: 49 frames')
