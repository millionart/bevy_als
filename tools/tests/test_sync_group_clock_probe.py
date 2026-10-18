import json
import struct
import sys
from pathlib import Path

f32 = lambda x: struct.unpack('f', struct.pack('f', x))[0]
data = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
assert 'sync_group_clock_probe' in data, 'Native synchronization group evidence is required'
probe = data['sync_group_clock_probe']
request = json.loads(Path(__file__).with_name('fixtures').joinpath('sync_group_clock.request.json').read_text())
assert len(probe['players']) == len(request['players']) == 4
assert len(probe['samples']) == len(request['samples'])
count = 0
for sample, row in zip(request['samples'], probe['samples']):
    assert row['name'] == sample['name'] and len(row['frames']) == len(sample['steps'])
    times = [f32(p['start']) for p in request['players']]
    prior = [0.0] * 4
    movement = [0.0] * 4
    valid = [False] * 4
    for step, frame in zip(sample['steps'], row['frames']):
        weights = step['weights']
        order = [i for i, w in enumerate(weights) if w is not None]
        for end in range(len(order)-1, 0, -1):
            lowest = 0
            for i in range(1, end+1):
                if f32(weights[order[lowest]]) > f32(weights[order[i]]):
                    lowest = i
            order[lowest], order[end] = order[end], order[lowest]
        assert frame['order'] == order
        assert frame['leader'] == (order[0] if order else None)
        if order:
            leader = order[0]
            length = probe['players'][leader]['duration']
            prior[leader] = times[leader]
            movement[leader] = f32(f32(request['players'][leader]['rate'] * probe['players'][leader]['rate_scale']) * f32(step['delta_seconds']))
            times[leader] = min(max(f32(times[leader] + movement[leader]), 0.0), length) if movement[leader] else times[leader]
            valid[leader] = True
            ratio = f32(times[leader] / length)
            previous_ratio = f32(prior[leader] / length)
            assert frame['ratio'] == ratio and frame['previous_ratio'] == previous_ratio
            for follower in order[1:]:
                length = probe['players'][follower]['duration']
                prior[follower] = f32(previous_ratio * length)
                times[follower] = f32(ratio * length)
                movement[follower] = f32(times[follower] - prior[follower])
                valid[follower] = True
        for index, player in enumerate(frame['players']):
            assert player == {'position': times[index], 'previous': prior[index], 'playback_delta': movement[index], 'has_previous': valid[index], 'ticked': weights[index] is not None}
        count += 1
assert count == 15
print(f'Native group ordering, joining, release, zero-weight and clock controls PASS: {count} frames')
