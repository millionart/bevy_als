import json
import sys
from pathlib import Path

probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['blend_list_probe']
frames = probe['samples'][0]['frames']
assert len(frames) == 320
assert all(frame['initializations'] == [1, 1] and frame['cached_initializations'] == 1 for frame in frames)
assert frames[0]['two_way']['initializations'] == [1, 1]
assert frames[20]['two_way']['initializations'] == [1, 2]
assert frames[40]['two_way']['initializations'] == [2, 2]
assert frames[240]['two_way']['initializations'] == [2, 3]
assert frames[20]['two_way']['alpha'] == 1
assert 0 < frames[240]['two_way']['alpha'] < 1
for frame in frames:
    nested = frame['two_way']['updates']
    expected = [2 + i for i, values in enumerate(nested) if values]
    if frame['cached_updates']:
        expected.append(0)
    assert frame['update_order'] == expected
    assert frame['cached_updates'] == frame['updates'][0]
assert all(frame['two_way']['updates'] == [[], []] and frame['two_way']['initializations'] == [2, 2] for frame in frames[220:240])
print('Native nested TwoWay activation, retained parent state, child reinitialization and deferred cache order PASS: 320 frames')
