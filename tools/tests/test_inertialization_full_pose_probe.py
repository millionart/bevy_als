import json
import math
import sys
from pathlib import Path

probe = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['inertialization_probe']
names = probe['bone_names']
assert len(names) == 79 and len(set(names)) == len(names)
assert names[0] == 'root' and probe['parent_indices'][0] == -1
assert all(-1 <= parent < index for index, parent in enumerate(probe['parent_indices']))
assert (probe['enabled'], probe['ignore_velocity'], probe['ignore_deficit']) == (1, 0, 0)
rows = {row['name']: row['frames'] for row in probe['samples']}
assert set(rows) == {'full_half_decay', 'no_request'}
for frames in rows.values():
    assert len(frames) == 3
    assert all(len(frame['bone_transforms']) == len(names) for frame in frames)
    assert all(frame['transform'] == frame['bone_transforms'][0] for frame in frames)
for name, amount in [('root', 100), ('pelvis', 20), ('foot_r', -40)]:
    index = names.index(name)
    for sample, factor in [('full_half_decay', 0.5), ('no_request', 1)]:
        frames = rows[sample]
        start = frames[0]['bone_transforms'][index]['translation']['x']
        end = frames[-1]['bone_transforms'][index]['translation']['x']
        assert abs(end - start - amount * factor) < 1e-5, (sample, name, start, end)
index = names.index('spine_02')
for sample, amount in [('full_half_decay', 0.1), ('no_request', 0.2)]:
    frames = rows[sample]
    start = frames[0]['bone_transforms'][index]['scale']['x']
    end = frames[-1]['bone_transforms'][index]['scale']['x']
    assert abs(end - start - amount) < 1e-6
index = names.index('foot_l')
for sample, angle in [('full_half_decay', math.pi / 4), ('no_request', math.pi / 2)]:
    frames = rows[sample]
    q = [frames[0]['bone_transforms'][index]['rotation'][key] for key in 'xyzw']
    a, b = math.sin(angle / 2), math.cos(angle / 2)
    expected = [b*q[0]+a*q[2], b*q[1]+a*q[3], b*q[2]-a*q[0], b*q[3]-a*q[1]]
    actual = [frames[-1]['bone_transforms'][index]['rotation'][key] for key in 'xyzw']
    dot = abs(sum(x*y for x, y in zip(expected, actual))) / math.sqrt(sum(x*x for x in expected)*sum(y*y for y in actual))
    assert 2*math.acos(min(1.0, dot)) < 2e-5
print('Native full-pose mapping, root/child translation, child rotation and scale PASS: 79 bones, 6 frames')
