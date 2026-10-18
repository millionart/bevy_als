import json
import math
import struct
import sys
from pathlib import Path

f = lambda value: struct.unpack('f', struct.pack('f', value))[0]
request = json.loads(Path(__file__).with_name('fixtures').joinpath('sequence_loop.request.json').read_text())['samples']
probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sequence_clock_probe']
assert len(probe['samples']) == len(request)
for sample, row in zip(request, probe['samples']):
    assert row['name'] == sample['name']
    assert len(row['frames']) == len(sample['deltas'])
    position = f(sample['start'])
    for delta, actual in zip(sample['deltas'], row['frames']):
        previous = position
        movement = f(f(delta) * f(f(sample['rate']) * probe['rate_scale']))
        position = f(previous + movement)
        if position < 0 or position > probe['duration']:
            position = f(math.fmod(position, probe['duration']))
            if position < 0:
                position = f(position + probe['duration'])
        assert abs(actual['position'] - position) < 1e-6
        assert abs(actual['previous'] - previous) < 1e-6
        assert abs(actual['playback_delta'] - movement) < 1e-6
        assert actual['has_previous']
    assert row['looping']
assert probe['samples'][1]['frames'][0]['position'] == probe['duration']
assert probe['samples'][1]['frames'][1]['position'] == probe['duration']
print('Native looping sequence clock and exact endpoint PASS')
