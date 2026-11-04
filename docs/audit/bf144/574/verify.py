import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('baseline', 'band')
phases = ('before', 'transaction', 'physics', 'final')
source = root.parent / '572'


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


assert read(root / 'compile-height.report.json')['success']
assert not read(root / 'compile-missing-import.report.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
native = next(case['frames'] for case in read(source / 'pelvis.native.json')['character_animation_probe']['cases']
              if case['name'] == 'coarse')
for frame in (52, 53):
    row = native[frame]
    assert row['world_delta_seconds'] == .125 and not row['blocking_hits']
    assert row['movement_before_tick']['movement_mode_name'] == 'MOVE_Walking'
    after = row['movement_after_tick']
    assert after['movement_mode_name'] == 'MOVE_Walking' and after['walkable_floor']
    assert after['floor_component'] == 'AuditLandingDeck'
    assert 1.9 < after['floor_distance_cm'] < 2.4
    assert abs(row['actor_transform']['translation']['z'] - native[51]['actor_transform']['translation']['z']) < 5e-6

observations, trajectory = {}, {}
for label in labels:
    path = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert not functional['pass'] and functional['images'] == []
    assert set(functional['issues']) == {'faults is non-empty', 'temporal_anomalies is non-empty'}
    assert read(path / 'process.report.json')['success']
    report = read(path / 'report.json')
    assert not report['graph_errors'] and report['primary_failure'] is None and report['screenshots'] == []
    warning, = report['temporal_anomalies']
    assert warning['kind'] == 'local_translation_spike' and warning['subject'] == 'pelvis'
    cache = read(path / 'mixed-before.json')
    assert cache['walking_acceleration'] == cache['cached_acceleration'] == 20
    states = {}
    for frame in (52, 53):
        states[frame] = {phase: read(path / f'height-{phase}-{frame}.json') for phase in phases}
        before, txn, physics, final = [states[frame][phase] for phase in phases]
        assert before['mode'] == final['mode'] == 'Grounded'
        assert before['delta'] == .125 and before['substeps'] == 3
        assert before['support'] == [0, 0, 0]
        assert txn['floor'] == 'AuditLandingDeck' and txn['floor_impact'] == 1.5
        assert txn['body_velocity_y'] == txn['motor_boost_y'] == physics['velocity'][1] == final['velocity'][1] == 0
        assert txn['motor_acceleration_y'] == -txn['gravity_y']
        assert txn['submitted'] == physics['position'] == final['position']
        assert txn['candidate'][::2] == txn['submitted'][::2]
        assert txn['control'] == (label == 'band')
        if label == 'band':
            assert txn['within_band'] and txn['submitted'][1] == before['position'][1]
        else:
            assert txn['candidate'] == txn['submitted']
    observations[label] = states
    rows = {row['ordinal']: row for row in read(path / 'parameters.json')['after']}
    trajectory[label] = {}
    for frame in range(52, 59):
        row = rows[frame]
        src = native[frame]
        position = src['actor_transform']['translation']
        speed = src['movement_after_tick']['velocity_cm_s']
        assert abs(row['position'][0] - position['x'] / 100) < 5e-6
        assert abs(row['velocity'][1] - speed['z'] / 100) < 2e-6
        expected = 'Air' if 54 <= frame <= 57 else 'Grounded'
        assert row['gameplay'] == expected
        gap = row['position'][1] - (2 + position['z']/100)
        trajectory[label][frame] = dict(position=row['position'], velocity=row['velocity'],
                                        vertical_gap=gap, mode=expected)

first_base, first_band = [observations[label][52] for label in labels]
assert first_base['before'] == first_band['before']
assert first_base['transaction']['candidate'] == first_band['transaction']['candidate']
assert first_base['transaction']['within_band']
initial_lift = first_base['transaction']['candidate'][1] - first_base['before']['position'][1]
assert .00252 < initial_lift < .00253
for frame in range(53, 58):
    before = trajectory['baseline'][frame]['vertical_gap']
    after = trajectory['band'][frame]['vertical_gap']
    assert .00249 < before < .00251 and 0 < after < .00007

print('Verified the multistep pre-physics lift and its unchanged Falling time budget', flush=True)
paths = [root / f'runtime-{label}' / f'{name}.json'
         for label in labels for name in ('parameters', 'report')]
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external = []
for name in ('572/pelvis.native.json.gz', '572/pelvis.request.json', '572/evidence-manifest.json',
             '573/observer.py', '573/cached_acceleration.inc', '573/summary.json'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_MULTISTEP_IN_BAND_LIFT_PROPAGATES_TO_FALLING', findings=['BF144-D273'],
               new_differences=0, total_differences=310, initial_lift=initial_lift,
               phase_observations=observations, trajectory=trajectory, public_functional_pass=False,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
