import json
import sys
from pathlib import Path


def check(directory):
    directory = Path(directory)
    labels = ['number', 'string', 'null', 'array', 'montage']
    for label in labels:
        output = directory / f'invalid-{label}.native.json'
        report = json.loads(Path(str(output) + '.reader.report.json').read_text(encoding='utf-8-sig'))
        assert report['exit_reason'] == 'completed' and report['process_exit_code'] == 7
        assert not report['success'] and not output.exists(), label
    return {'invalid_capture_flag_types':4, 'unsupported_pose_assets':1, 'stale_outputs':0}


if __name__ == '__main__':
    print(json.dumps(check(sys.argv[1])))
