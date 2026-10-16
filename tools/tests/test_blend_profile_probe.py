import json
import math
import sys
from pathlib import Path

request = json.loads(Path(__file__).with_name('fixtures').joinpath('blend_profile.request.json').read_text())
data = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['blend_profile_probe']['samples']
assert len(data) == len(request['samples']) == 6
for sample, result in zip(request['samples'], data):
    assert result['name'] == sample['name']
    assert len(result['frames']) == 1
    frame = result['frames'][0]
    alpha = frame['alpha']
    factor = sample['deltas'][0] / sample['duration']
    assert frame['complete'] == (factor >= 1.0)
    assert abs(frame['blend_alpha'] - min(factor, 1.0)) < 1e-7
    assert abs(frame['blend_value'] - alpha) < 1e-7
    assert abs(alpha - factor * factor * (3 - 2 * factor)) < 1e-7
    assert len(frame['bones']) == 19
    scale = {'QuickFeetBlend': 5.0, 'MoveDirectionChange': 2.0}[sample['profile']]
    for name, bone in frame['bones'].items():
        if name == 'default':
            first, second = 1 - alpha, alpha
        else:
            first, second = max((1 - alpha) / scale, 1e-5), max(alpha * scale, 1e-5)
            total = first + second
            if abs(total - 1) > 1e-5:
                first, second = first / total, second / total
        assert abs(bone['source_weight'] - first) < 1e-7
        assert abs(bone['target_weight'] - second) < 1e-7
        for channel, source_key in [('translation', 'translation_cm'), ('scale', 'scale')]:
            for i, axis in enumerate('xyz'):
                expected = sample['from'][source_key][i] * first + sample['to'][source_key][i] * second
                assert abs(bone['transform'][channel][axis] - expected) < 3e-6
        expected_rotation = [a * first + b * second for a, b in zip(sample['from']['rotation'], sample['to']['rotation'])]
        length = math.sqrt(sum(v * v for v in expected_rotation))
        assert max(abs(bone['transform']['rotation'][axis] - expected_rotation[i] / length) for i, axis in enumerate('xyzw')) < 1e-7
    assert abs(frame['curve'] - (sample['curve_a'] * (1 - alpha) + sample['curve_b'] * alpha)) < 1e-6
print('Native blend profile weights, default bones, transforms, and curves PASS')
