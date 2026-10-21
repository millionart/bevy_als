import json
import sys
from pathlib import Path

from test_sync_group_pose_probe import check, check_sequence_oracle


def strip_pose(value):
    if isinstance(value, dict):
        return {key: strip_pose(item) for key, item in value.items()
                if key not in ['pose', 'pose_evaluation_mode']}
    if isinstance(value, list):
        return [strip_pose(item) for item in value]
    return value


def check_modes(document, expected):
    modes = []
    for row in document['sync_group_clock_probe']['samples']:
        for frame in row['frames']:
            for player in frame['players']:
                if player['ticked']:
                    modes.extend(sample['pose_evaluation_mode'] for sample in player.get('samples', []))
                    if 'samples' not in player:
                        modes.append(player['pose_evaluation_mode'])
    assert modes and set(modes) == {expected}, modes
    return len(modes)


def run(directory):
    root = Path(directory)
    read = lambda path: json.loads(path.read_text(encoding='utf-8-sig'))
    documents = {name: read(root / (name + '.native.json')) for name in ['default', 'false', 'prepared']}
    assert documents['default'] == documents['false']
    assert strip_pose(documents['default']) == strip_pose(documents['prepared'])
    assert documents['default'] != documents['prepared']
    for name in ['default', 'false', 'prepared']:
        check(documents[name])
    raw = check_modes(documents['default'], 'raw_model')
    compressed = check_modes(documents['prepared'], 'compressed')
    oracle = read(root / 'oracle-prepared.native.json')
    result = check_sequence_oracle(documents['prepared'], oracle)
    try:
        check_sequence_oracle(documents['default'], oracle)
    except AssertionError:
        pass
    else:
        raise AssertionError('A mismatched raw/compressed oracle was accepted')
    names = ['invalid-no-capture'] + [f'invalid-{kind}-{name}' for kind in ['group', 'sequence']
                                    for name in ['number', 'string', 'null', 'array']]
    for name in names:
        output = root / (name + '.native.json')
        report = read(Path(str(output) + '.reader.report.json'))
        assert report['exit_reason'] == 'completed' and report['process_exit_code'] == 7, name
        assert not report['success'] and not output.exists(), name
    return {'raw_evaluations': raw, 'compressed_evaluations': compressed,
            'default_and_false_exact': True, 'clock_fields_exact': True,
            'mismatched_oracle_rejected': True, 'invalid_requests_rejected': len(names), 'oracle': result}


if __name__ == '__main__':
    print(json.dumps(run(sys.argv[1])))
