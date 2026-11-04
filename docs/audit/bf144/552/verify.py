import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '551'
labels = ('baseline', 'stop', 'restored')


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def frame(report, ordinal):
    row, = [row for row in report['after'] if row['ordinal'] == ordinal]
    return row


def reflected(fields, name):
    field, = [field for field in fields if field['name'] == name]
    return field


def native_state(row):
    fields = row['source_state_after_tick']
    motion = reflected(fields, 'LocomotionState')['fields']
    grounded = reflected(fields, 'GroundedState')['fields']
    blend = reflected(grounded, 'VelocityBlend')['fields']
    return dict(
        has_input=reflected(motion, 'bHasInput')['exported_text'] == 'True',
        moving=reflected(motion, 'bMoving')['exported_text'] == 'True',
        moving_smooth=reflected(motion, 'bMovingSmooth')['exported_text'] == 'True',
        speed_m_s=reflected(motion, 'Speed')['numeric_value'] / 100,
        velocity_blend=[reflected(blend, direction + 'Amount')['numeric_value']
                        for direction in ('Forward', 'Backward', 'Left', 'Right')],
    )


assert read(root / 'compile-handoff.report.json')['success']
source_file = prior / 'stall.native.json.gz'
case = next(c for c in read(source_file)['character_animation_probe']['cases'] if c['name'] == 'stalled')
source = {q: native_state(case['frames'][q]) for q in (39, 40, 41)}
assert source[39]['velocity_blend'] == source[40]['velocity_blend'] == [0, 0, 0, 0]
assert not source[39]['has_input'] and source[40]['has_input']
assert not source[40]['moving'] and not source[40]['moving_smooth'] and source[40]['speed_m_s'] == 0
assert source[41]['moving'] and source[41]['speed_m_s'] > .01
native_q40 = case['frames'][40]
assert native_q40['actor_velocity_cm_s'] == {'x': 0, 'y': 0, 'z': 0}
first_hit, = native_q40['blocking_hits']
assert first_hit['time'] == 0 and first_hit['move_stop_cm'] == first_hit['trace_start_cm']
native_step_input_x = (first_hit['trace_end_cm']['x'] - first_hit['trace_start_cm']['x']) / (native_q40['world_delta_seconds'] / 2) / 100

prior_dir = prior / 'runtime-stalled'
prior_q40 = frame(read(prior_dir / 'parameters.json'), 40)
unchanged = ('fixture', 'stall-wall', 'stall-substeps', 'walking-step', 'stage-before',
             'stage-after', 'walking-transaction')
for name in unchanged:
    assert load('baseline', name) == load('stop', name) == load('restored', name)
    assert load('baseline', name) == read(prior_dir / f'{name}.json')

target = {}
for label in labels:
    stop = label == 'stop'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    one, two = load(label, 'stall-substeps')['rows']
    assert one['start'] == one['end'] and one['applied'] == [0, 0, 0]
    assert abs(one['input_velocity'][0] - native_step_input_x) < 1e-6
    handoff = load(label, 'stop-handoff')
    assert handoff['enabled'] == stop
    assert handoff['original_position'] == two['end']
    assert handoff['original_pre_collision'] == two['input_velocity']
    assert handoff['original_feedback'] == load(label, 'stage-after')['collision_velocity']
    assert handoff['selected_position'] == (one['end'] if stop else two['end'])
    assert handoff['selected_pre_collision'] == (one['input_velocity'] if stop else two['input_velocity'])
    assert handoff['selected_feedback'] == ([0, 0, 0] if stop else handoff['original_feedback'])
    for key in ('motor_boost', 'motor_acceleration', 'raw_velocity'):
        assert handoff[key] == load('baseline', 'stop-handoff')[key]
    result = load(label, 'contact-after-40')
    assert result['mode'] == 'Grounded' and result['transitions'] == []
    assert result['position'] == handoff['selected_position']
    motion = load(label, 'locomotion-consumer')
    assert motion['ordinal'] == 40 and motion['has_input']
    assert motion['raw_velocity'] == motion['velocity'] == result['published'] == handoff['selected_feedback']
    assert motion['speed'] == motion['graph_speed']
    assert motion['moving'] == motion['moving_smooth'] == (not stop)
    report = load(label, 'parameters')
    q39, q40, q41 = [frame(report, q) for q in (39, 40, 41)]
    assert q39['velocity_blend']['amounts'] == [0, 0, 0, 0]
    assert q40['graph_error'] is None and q40['gameplay'] == 'Grounded'
    assert q40['velocity'] == q40['velocity_blend']['velocity'] == motion['velocity']
    assert q40['velocity_blend']['calls'] and q40['velocity_blend']['write_frame'] is not None
    if stop:
        assert motion['speed'] == source[40]['speed_m_s']
        assert q40['velocity_blend']['amounts'] == source[40]['velocity_blend']
    else:
        assert motion['speed'] > .01
        assert q40['velocity_blend'] == prior_q40['velocity_blend']
        assert q40['velocity_blend']['amounts'][0] > .2
    target[label] = dict(handoff=handoff, consumer=motion, position=result['position'],
                         velocity_blend=q40['velocity_blend'], next_velocity=q41['velocity'])
assert target['baseline'] == target['restored']
for name in ('contact-after-40', 'contact-after-41', 'locomotion-consumer'):
    assert load('baseline', name) == load('restored', name)
print('Verified real movement flags and VelocityBlend after stopping at the zero-motion substep', flush=True)

decompressed = {}
for label in labels:
    for name in ('parameters', 'report', 'walking-step', 'walking-transaction'):
        path = root / f'runtime-{label}' / f'{name}.json'
        data = raw(path)
        archive = path.with_suffix('.json.gz')
        if path.exists():
            archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
        assert gzip.decompress(archive.read_bytes()) == data
        decompressed[str(archive.relative_to(root))] = digest(data)
        path.unlink(missing_ok=True)

external_paths = [source_file, prior / 'observer.py', prior / 'target_wall.inc', prior / 'substeps.inc',
                  prior_dir / 'parameters.json.gz', *(prior_dir / f'{n}.json' for n in unchanged)]
external = []
for p in external_paths:
    if not p.exists():
        p = p.with_suffix('.json.gz')
    external.append(dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())))
summary = dict(gate='PASS_D305_ZERO_SUBSTEP_HANDOFF_LOCOSPEED_MOVING_AND_BLEND', supplements=['BF144-D305'],
               new_differences=0, total_differences=302, ordinal=40, native=source, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
