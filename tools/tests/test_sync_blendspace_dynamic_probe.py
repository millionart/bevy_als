import json
import math
import sys
from pathlib import Path

probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
assert len(probe['players']) == 2 and len(probe['samples']) == 1
frames = probe['samples'][0]['frames']
assert len(frames) == 44
for frame in frames:
    assert frame['leader'] == 0 and frame['order'] == [0, 1]
    assert frame['marker_start']['valid'] and frame['marker_end']['valid']
    for player in frame['players']:
        assert player['ticked'] and player['has_previous']
        assert 0 <= player['position'] <= 1
        assert abs(sum(sample['weight'] for sample in player['samples']) - 1) < 1e-6
        assert all(math.isfinite(sample['time']) and math.isfinite(sample['previous_time']) for sample in player['samples'])
        assert .19999 <= player['filtered_position']['x'] <= 1.00001
        assert -.00001 <= player['filtered_position']['y'] <= 1.00001
for index, positions in [(0, [[1, 0], [.5, 1]]), (19, [[.2, 1], [1, 0]]), (31, [[.4, 1], [.5, 1]]), (43, [[1, 0], [.3, 1]])]:
    for player, expected in zip(frames[index]['players'], positions):
        assert all(abs(player['filtered_position'][axis] - value) < 1e-6 for axis, value in zip(['x', 'y'], expected))
assert .2 < frames[8]['players'][0]['filtered_position']['x'] < 1
assert 0 < frames[8]['players'][0]['filtered_position']['y'] < 1
assert .4 < frames[32]['players'][0]['filtered_position']['x'] < 1
assert 0 < frames[32]['players'][0]['filtered_position']['y'] < 1
assert frames[8]['players'][0]['filtered_position'] != frames[8]['players'][1]['filtered_position']
print('Native dynamic axes, independent filters, fixed fallback and synchronized clocks PASS: 44 frames')
