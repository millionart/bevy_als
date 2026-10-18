import json
import sys
from pathlib import Path

probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
assert len(probe['players']) == 2
rows = {row['name']: row['frames'] for row in probe['samples']}
assert set(rows) == {'cold_zero', 'warm_zero', 'cold_tiny', 'warm_tiny'}
zero = rows['cold_zero'][0]
tiny = rows['cold_tiny'][0]
for frame in [zero, tiny]:
    assert frame['leader'] == 0 and frame['marker_start']['valid'] and frame['marker_end']['valid']
    assert all(sample['previous_time'] == 0 for sample in frame['players'][0]['samples'])
assert zero['players'][0]['samples'] == tiny['players'][0]['samples']
assert all(sample['time'] == sample['previous_time'] == 0 for sample in zero['players'][1]['samples'])
assert zero['players'][1]['position'] == 0
assert all(sample['time'] > 0 for sample in tiny['players'][1]['samples'])
for name in ['warm_zero', 'warm_tiny']:
    frames = rows[name]
    for frame in frames[3:5]:
        assert frame['leader'] == 1
        assert not frame['marker_start']['valid'] and not frame['marker_end']['valid']
        for before, after in zip(frames[2]['players'], frame['players']):
            assert after['position'] == before['position']
            assert after['samples'] == before['samples']
    assert frames[5]['leader'] == 0 and frames[5]['marker_end']['valid']
assert sum(map(len, rows.values())) == 17
print('Native BlendSpace cold/warm zero and near-zero leader/follower lifecycles PASS: 17 frames')
