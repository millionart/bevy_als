import json
import sys
from pathlib import Path

probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['blend_list_probe']
sample = next(row for row in probe['samples'] if row['name'] == '0.016666666666666666:partial_requests')
frames = sample['frames']
assert len(frames) == 156
dual = blocked = 0
for index, frame in enumerate(frames):
    requests = frame['cached_requests']
    assert frame['cached_initializations'] == 1
    assert frame['cached_updates'] == ([max(requests)] if requests else [])
    expected = [2 + child for child, values in enumerate(frame['two_way']['updates']) if values]
    if requests:
        expected.append(0)
    assert frame['update_order'] == expected
    if len(requests) == 2:
        dual += 1
        assert max(requests) < sum(requests)
    if frame['outer_alpha'] == 1:
        blocked += 1
        assert frame['updates'] == [[], []]
        assert frame['two_way']['updates'] == [[], []]
        assert frame['cached_updates'] == [1]
        previous = frames[index - 1]
        assert frame['weights'] == previous['weights']
        assert frame['two_way']['alpha'] == previous['two_way']['alpha']
        assert frame['two_way']['initializations'] == previous['two_way']['initializations']
assert dual > 0 and blocked == 12
print(f'Native outer TwoWay shared cache maximum weight, single initialization and blocked state PASS: {len(frames)} frames, {dual} dual requests')
