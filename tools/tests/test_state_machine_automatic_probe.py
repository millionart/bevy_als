import json
import struct
import sys
from pathlib import Path

f32 = lambda value: struct.unpack('f', struct.pack('f', value))[0]
report = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
probe = report['state_machine_entry_probe']
assert probe['machine_index'] == 0
assert len(probe['samples']) == 4
for sample in probe['samples']:
    weight, position = map(float, sample['name'].split(':'))
    frames = sample['frames']
    assert sample['max_transitions'] == 3
    assert len(frames) == 3
    assert frames[0]['state'] == 1 and frames[0]['weights'] == [0, 1, 0, 0, 0, 0]
    assert frames[1]['state'] == 4 and frames[1]['weights'] == [0, 0, 0, 0, 1, 0]
    remaining = f32(f32(1.1666666269302368) - f32(position))
    eligible = weight > 0 and remaining <= f32(.3)
    assert frames[2]['state'] == (0 if eligible else 4)
    if eligible and remaining > 0:
        alpha = f32(f32(1 / 60) / remaining)
        alpha = f32(f32(alpha * alpha) * f32(3 - f32(2 * alpha)))
        assert frames[2]['weights'] == [alpha, 0, 0, 0, f32(1 - alpha), 0]
        assert frames[2]['active_transitions'] == 1
    else:
        assert frames[2]['active_transitions'] == 0
        assert frames[2]['weights'] == ([1, 0, 0, 0, 0, 0] if eligible else [0, 0, 0, 0, 1, 0])
    assert all(frame['updates'][5] == [] for frame in frames)
print('All four native automatic-rule controls and 12 update frames match.', flush=True)
