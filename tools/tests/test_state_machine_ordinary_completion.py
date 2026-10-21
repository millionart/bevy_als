import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
probe = report['state_machine_entry_probe']
assert probe['machine_index'] == 0 and len(probe['samples']) == 2
for sample in probe['samples']:
    assert sample['name'] in ['30', '60']
    frames = sample['frames']
    assert len(frames) == 20
    assert frames[0]['state'] == 1 and frames[1]['state'] == 3
    end = next(i for i in range(3, len(frames)) if frames[i - 1]['active_transitions'] and not frames[i]['active_transitions'])
    assert all(frame['state'] == 4 for frame in frames[2:])
    assert all(frame['updates'][3] and frame['updates'][4] for frame in frames[2:end])
    assert all(frame['updates'][3] == [] and frame['updates'][4] == [1] for frame in frames[end:])
    assert all(weight > 0 for frame in frames for weight in frame['updates'][3])
print('Native ordinary completion stops updating the old state on the completion frame at both 30 and 60 Hz.', flush=True)
