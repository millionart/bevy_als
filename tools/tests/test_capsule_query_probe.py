import json
import math
import sys
from pathlib import Path


def check(path):
    data = json.loads(Path(path).read_text(encoding='utf-8-sig'))
    samples = {sample['input']['name']: sample['capsule_sweep'] for sample in data['geometry_query_probe']['samples']}
    assert len(samples) == 7
    for name, result in samples.items():
        assert result['hit'] == (name not in {'sphere_control', 'air_control'}), name
        if result['hit']:
            assert math.isfinite(result['distance_cm']) and result['distance_cm'] > 0, name
            assert all(math.isfinite(value) for field in ['position_cm', 'normal'] for value in result[field].values()), name
            assert abs(result['normal']['z'] - 1.0) < 1e-5, name
    assert abs(samples['axis']['distance_cm'] - 4.0) < 1e-5
    for index in range(2):
        assert abs(samples[f'floor_4_step_{index}']['distance_cm'] - samples[f'floor_100_step_{index}']['distance_cm']) < 1e-5
    print('Capsule query probe self-test: PASS', flush=True)


if __name__ == '__main__':
    check(sys.argv[1])
