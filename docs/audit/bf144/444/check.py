import copy
import json
import math
import sys
from pathlib import Path

r = Path(__file__).resolve().parent


def validate(data):
    phases = []
    for key in ['animation_before', 'animation_after']:
        rows = [v for v in data[key] if v['ordinal'] is not None]
        assert [v['ordinal'] for v in rows] == list(range(240))
        for row in rows:
            assert row['arena_frame'] is not None
            assert row['animation_delta'] == row['frame_delta']
            assert row['virtual_delta_ns'] == 33333335
            assert row['provider_velocity'] == row['motion_world_velocity'] == row['velocity']
            assert row['provider_speed'] == row['motion_speed']
            assert row['provider_has_input'] == row['motion_has_input']
            assert row['graph_error'] is None and not row['paused']
            assert row['gate']['update_graph']
            assert math.isfinite(row['elapsed'])
        phases.append(rows)
    for before, after, final, motor in zip(*phases, data['camera_after_post_update'], data['after_motors'], strict=True):
        assert after['arena_frame'] == before['arena_frame'] + 1
        assert before['position'] == after['position'] == final['position']
        assert before['velocity'] == after['velocity'] == final['velocity']
        assert after['elapsed'] == after['pose_timestamp']
        assert before['ordinal'] == final['ordinal'] == motor['ordinal']
    return dict(animation_input_rows_exact=240, before_and_after_evaluation_rows=480)


if __name__ == '__main__':
    label = sys.argv[1]
    if label == 'self-test':
        d = dict(animation_before=[], animation_after=[], camera_after_post_update=[], after_motors=[])
        for i in range(240):
            row = dict(ordinal=i, frame_delta=1/30, animation_delta=1/30, virtual_delta_ns=33333335, arena_frame=i,
                       provider_velocity=[1,0,0], motion_world_velocity=[1,0,0], velocity=[1,0,0], position=[0,1,0],
                       provider_speed=1, motion_speed=1, provider_has_input=True, motion_has_input=True,
                       graph_error=None, paused=False, gate=dict(update_graph=True), elapsed=0, pose_timestamp=0)
            d['animation_before'].append(row)
            after = copy.deepcopy(row)
            after['arena_frame'] += 1
            d['animation_after'].append(after)
            d['camera_after_post_update'].append(dict(ordinal=i, position=row['position'], velocity=row['velocity']))
            d['after_motors'].append(dict(ordinal=i))
        validate(d)
        for kind in range(4):
            bad=copy.deepcopy(d)
            if kind == 0:
                del bad['animation_before']
            elif kind == 1:
                bad['animation_after'][20]['arena_frame'] -= 1
            elif kind == 2:
                bad['animation_before'][20]['provider_velocity'][0] = 2
            else:
                bad['animation_after'][20]['animation_delta'] = 0
            try:
                validate(bad)
            except (AssertionError,KeyError):
                continue
            raise AssertionError(kind)
        result = dict(rejected=4)
    else:
        result = validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
