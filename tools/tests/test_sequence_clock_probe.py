import json
import struct
import sys
from pathlib import Path

f32 = lambda value: struct.unpack('f', struct.pack('f', value))[0]
request = json.loads(Path(__file__).with_name('fixtures').joinpath('sequence_clock.request.json').read_text())
probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sequence_clock_probe']
assert len(probe['samples']) == len(request['samples']) == 4
for sample, result in zip(request['samples'], probe['samples']):
    assert sample['name'] == result['name']
    assert len(sample['deltas']) == len(result['frames'])
    time = f32(sample['start'])
    for delta, frame in zip(sample['deltas'], result['frames']):
        previous = time
        move = f32(f32(sample['rate'] * probe['rate_scale']) * f32(delta))
        time = min(max(f32(time + move), 0.0), probe['duration']) if move else time
        assert frame['position'] == time
        assert frame['previous'] == previous
        assert frame['playback_delta'] == move
        assert frame['has_previous']
        assert frame['ratio'] == f32(time / probe['duration'])
print('Native sequence zero-step, advance, clamp, and reverse clock controls PASS')
