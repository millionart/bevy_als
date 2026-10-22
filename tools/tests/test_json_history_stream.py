import json
import tempfile
from pathlib import Path

from json_history_stream import members


def self_test():
    case = {'declared_controls': None, 'frames': [{'frame': 0, 'snapshot': {'number': 12345.625, 'text': 'quote " comma, bracket } escaped \u0041', 'nested': [True, None, {'x': -1.25e-9}]}}, {'frame': 1, 'snapshot': {'number': 77}}], 'hz': 30, 'unknown_member': {'keep': [1, 2]}, 'warm_history': [{'value': 9}]}
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / 'history.json'
        for chunk in [1, 2, 7, 64, 65536, 4194304]:
            path.write_text(json.dumps([case], ensure_ascii=True))
            events = list(members(path, chunk))
            rebuilt = {'frames': []}
            for kind, key, value in events:
                if kind == 'frame':
                    assert key == len(rebuilt['frames'])
                    rebuilt['frames'].append(value)
                else:
                    rebuilt[key] = value
            assert rebuilt == case
        rejected = []
        for name, content in [('truncated', json.dumps([case])[:-3]), ('second_case', json.dumps([case, case])), ('trailing_frame_comma','[{"frames":[1,]}]'), ('trailing_case_comma','[{"frames":[],}]'), ('missing_frames', '[{"hz":30}]'), ('duplicate_member', '[{"frames":[],"hz":1,"hz":2}]')]:
            path.write_text(content)
            try:
                list(members(path, 3))
            except (ValueError, json.JSONDecodeError):
                rejected.append(name)
        assert len(rejected) == 6
    return {'self_test': 'PASS', 'chunk_sizes': 6, 'malformed_cases_rejected': rejected, 'all_case_and_frame_members_preserved': True}


if __name__ == '__main__':
    print(json.dumps(self_test()))
