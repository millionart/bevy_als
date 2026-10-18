import json
import struct
import sys
from pathlib import Path

f32 = lambda value: struct.unpack('f', struct.pack('f', value))[0]
probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sequence_clock_probe']
assert 'follower' in probe, 'Native follower clock evidence is required'
length = probe['follower']['duration']
assert length > 0 and probe['follower']['asset_path']
assert probe['follower']['equal_weight_order'] == [1, 0]
assert probe['follower']['leader_marker_count'] == probe['follower']['follower_marker_count'] == 0
frames = 0
for sample in probe['samples']:
    for frame in sample['frames']:
        follower = frame['follower']
        assert follower['has_previous']
        assert follower['position'] == f32(frame['ratio'] * length)
        assert follower['previous'] == f32(f32(frame['previous'] / probe['duration']) * length)
        assert follower['playback_delta'] == f32(follower['position'] - follower['previous'])
        frames += 1
assert frames >= 4
print(f'Native follower normalized positions and deltas PASS: {frames} frames')
