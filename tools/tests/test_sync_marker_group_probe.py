import json
import sys
from pathlib import Path

request = json.loads(Path(__file__).with_name('fixtures').joinpath('sync_marker_group.request.json').read_text())
probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
assert len(probe['players']) == len(request['players']) == 6
for player, expected in zip(probe['players'], request['players']):
    assert player['asset_path'] == expected['asset']
    assert {marker['name'] for marker in player['markers']} == {'Left', 'Right'}
    assert all(0 <= marker['time'] < player['duration'] for marker in player['markers'])

def position(player, marker):
    times = {value['name']: value['time'] for value in player['markers']}
    start, end = times[marker['previous']], times[marker['next']]
    if end <= start:
        end += player['duration']
    return (start + (end-start)*marker['alpha']) % player['duration']

assert len(probe['samples']) == len(request['samples'])
for sample, expected in zip(probe['samples'], request['samples']):
    assert sample['name'] == expected['name'] and len(sample['frames']) == len(expected['steps'])
    intervals = set()
    for frame in sample['frames']:
        assert frame['leader'] == 0
        assert len(frame['players']) == 6
        assert frame['marker_start']['valid'] and frame['marker_end']['valid']
        intervals.add((frame['marker_end']['previous'], frame['marker_end']['next']))
        for player, clock in zip(probe['players'], frame['players']):
            assert clock['has_previous'] and clock['ticked']
            assert abs(position(player, frame['marker_end']) - clock['position']) < 2e-6
            assert abs(position(player, frame['marker_start']) - clock['previous']) < 2e-6
    assert len(intervals) == 2
print('Native marker group ownership, crossing, and follower positions PASS')
