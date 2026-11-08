import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def main():
    root = Path('docs/audit/bf144/d253')
    rows = json.loads(Path('docs/audit/bf144/441c/kernel-inputs.json').read_text())
    row = next(value for value in rows if value['ordinal'] == 20)
    source = json.loads(Path('docs/audit/bf144/501/evidence-input.json').read_text())['source']
    step = f32(row['simulation_delta'] / 6)
    per_step = [f32(value * step) for value in row['planned_velocity']]
    accumulated = [0.0, 0.0, 0.0]
    for _ in range(6):
        accumulated = [f32(a + b) for a, b in zip(accumulated, per_step)]
    result = {
        'target_ordinal': row['ordinal'],
        'simulation_delta': row['simulation_delta'],
        'substeps': 6,
        'planned_velocity': row['planned_velocity'],
        'planned_delta': row['planned_delta'],
        'per_substep_model': per_step,
        'six_substep_accumulation_model': accumulated,
        'source_501': {key: source[key] for key in ('velocity', 'start', 'end', 'delta')},
    }
    (root / 'witness.json').write_text(json.dumps(result, indent=2) + '\n')
    print('Recorded source and six-substep float model', flush=True)


if __name__ == '__main__':
    main()
