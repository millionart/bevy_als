import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
probe = report['cached_pose_lifecycle_probe']
assert len(probe['frames']) == 9
assert len(probe['weight_cases']) == 5
for case in probe['weight_cases']:
    expected = [max(case['requests'])] if case['requests'] else []
    assert case['child_update_weights'] == expected
assert probe['weight_cases'][0]['child_update_weights'] == [1]
assert probe['weight_cases'][1]['child_update_weights'] == [1]
print('Native cached pose selects the maximum request once, in either order, including zero and empty requests.', flush=True)
