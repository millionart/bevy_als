import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def load(name):
    path = root / name
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())

def save(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def hermite(alpha):
    return f32(f32(alpha * alpha) * f32(3 - f32(2 * alpha)))

def native_replay(target, markers, length, mode):
    rate = f32(1.3)
    fade_limit = f32(0.3)
    position = alpha = time_left = 0.0
    remaining = duration = f32(0.1)
    fading = False
    playing = True
    for frame in target:
        delta = frame['delta']
        if remaining > delta:
            alpha = f32(alpha + f32(f32(f32(1 - alpha) / remaining) * delta))
            remaining = f32(remaining - delta)
            if fading and mode == 'no-alpha-history':
                alpha = f32(f32(duration - remaining) / duration)
        else:
            alpha, remaining = 1.0, 0.0
        weight = hermite(alpha)
        if fading:
            weight = hermite(f32(1 - alpha)) if mode == 'reverse-curve' else f32(1 - weight)
        if playing:
            time_left = f32(time_left + delta)
            for _ in range(9):
                if time_left <= 1e-8:
                    break
                move = f32(time_left * rate)
                marker = next((m for m in markers if position < m <= f32(position + move)), None)
                if marker is not None and mode != 'no-branching':
                    move = f32(marker - position)
                move = min(move, f32(length - position))
                position = f32(position + move)
                time_left = max(0.0, f32(time_left - f32(move / rate)))
                remaining_play = f32(f32(length - position) / rate)
                reached_fade = (frame['natural_blend_start'] is not None
                                if mode == 'elapsed-trigger' else remaining_play <= fade_limit)
                if not fading and reached_fade:
                    fading, alpha = True, 0.0
                    duration = (f32(f32(length / rate) - frame['time'])
                                if mode == 'elapsed-duration' else remaining_play)
                    remaining = duration
                if position == length:
                    position = f32(length - f32(0.00005))
                    playing = False
                    break
        if fading and remaining == 0 and alpha == 1:
            playing = False
        yield dict(position=position, weight=weight, stopped=fading, alpha=alpha,
                   blend_time_remaining=remaining, blend_time=duration)

clock = load('roll-clock.native.json')
notify = load('roll-notify.native.json')
for name in ('roll-clock', 'roll-notify'):
    assert load(name + '.native.json.reader.report.json')['success']
props = {p['name']: p for p in clock['asset_properties']}
assert props['RateScale']['numeric_value'] == 1
assert props['BlendOutTriggerTime']['numeric_value'] == -1
assert 'BlendTime=0.100000,BlendOption=HermiteCubic' in props['BlendIn']['exported_text']
assert 'BlendTime=0.300000,BlendOption=HermiteCubic' in props['BlendOut']['exported_text']
assert '0.000100' in props['BranchingPointMarkers']['exported_text']
assert '0.933433' in props['BranchingPointMarkers']['exported_text']
assert len([p for p in props['TimeStretchCurve']['fields'] if p['name'] == 'Markers' and p['exported_text'] == '']) == 1
event_times = {(e['trigger_time'], e['end_trigger_time'])
               for case in notify['notify_clock_probe']['cases'] for e in case['events']}
assert len(event_times) == 1
markers = next(iter(event_times))
length = clock['montage_clock_probe']['montage_length']
assert length == 1.5
source = clock['montage_clock_probe']['samples']
target = load('replay-baseline.json')
assert len(source) == len(target) == 14
runtime_source = load('../506/native-slot.json')
runtime_target = {row['ordinal']:row for row in load('../506/runtime-baseline/parameters.json.gz')['after']}
assert len(runtime_source) == 72
for case in ('compile-baseline', 'baseline'):
    assert load(f'replay-{case}.report.json')['success']

modes = ('source', 'no-branching', 'elapsed-trigger', 'elapsed-duration', 'no-alpha-history', 'reverse-curve')
keys = ('position', 'weight', 'stopped')
selected = sys.argv[1] if len(sys.argv) == 2 else 'source'
assert selected in modes
results = []
total_mismatches = 0
for native, current in zip(source, target, strict=True):
    name = native['name']
    assert name == current['name']
    s, t = native['frames'], current['frames']
    assert [x['delta_seconds'] for x in s] == [x['delta'] for x in t]
    variants = {mode: list(native_replay(t, markers, length, mode)) for mode in modes}
    differences = {mode: {key: [i for i, (a, b) in enumerate(zip(s, rows, strict=True)) if a[key] != b[key]]
                          for key in keys} for mode, rows in variants.items()}
    total_mismatches += sum(len(indices) for indices in differences[selected].values())
    source_trigger = next(i for i, row in enumerate(s) if row['stopped'])
    target_trigger = next(i for i, row in enumerate(t) if row['natural_blend_start'] is not None)
    source_resume = next(i for i in range(source_trigger + 1, len(s)) if s[i]['weight'] < 1)
    target_resume = next(i for i in range(target_trigger + 1, len(t)) if t[i]['weight'] < 1)
    if name == 'constant-30':
        for i, (a, b, replay) in enumerate(zip(s[:35], t[:35], variants['source'][:35], strict=True)):
            assert b['time'] == runtime_target[59+i]['roll']['time']
            assert b['weight'] == runtime_target[59+i]['post_locomotion']['weight']
            assert a['weight'] == runtime_source[25+i]['slot_weight']
            if montage := runtime_source[25+i]['roll']:
                assert (a['position'], a['weight']) == (montage['position'], montage['blend_value'])
                assert (replay['alpha'], replay['blend_time_remaining'], replay['blend_time']) == (
                    montage['blend_alpha'], montage['blend_time_remaining'], montage['blend_time'])
    results.append(dict(name=name, frames=len(s), source_trigger=source_trigger, target_trigger=target_trigger,
                        source_resume=source_resume, target_resume=target_resume,
                        source_weight_at_target_resume=s[target_resume]['weight'],
                        target_weight_at_target_resume=t[target_resume]['weight'],
                        target_weight_mismatches=[i for i,(a,b) in enumerate(zip(s,t,strict=True)) if a['weight'] != b['weight']],
                        ablation_mismatches=differences))
    print('Verified Roll clock sample ' + name, flush=True)
if selected != 'source':
    save(f'negative-{selected}.json', dict(mode=selected, total_mismatches=total_mismatches))
    assert total_mismatches == 0, selected
assert total_mismatches == 0
edges = [row for row in results if row['source_resume'] != row['target_resume']]
assert [row['name'] for row in edges] == ['fade-edge-5', 'fade-edge-6']
assert all(row['source_resume'] == 27 and row['target_resume'] == 26 for row in edges)
assert all(row['source_weight_at_target_resume'] == 1 and row['target_weight_at_target_resume'] < 1 for row in edges)
original = next(row for row in results if row['name'] == 'constant-30')
assert original['target_weight_mismatches'] == list(range(26, 34))
assert original['ablation_mismatches']['elapsed-duration']['weight']
assert original['ablation_mismatches']['no-alpha-history']['weight']
assert original['ablation_mismatches']['reverse-curve']['weight']
assert original['ablation_mismatches']['no-branching']['position']
save('comparison.json', results)
save('final-source.json', dict(sha256=hashlib.sha256((root / 'replay.rs').read_bytes()).hexdigest()))
