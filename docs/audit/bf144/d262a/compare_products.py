import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def runtime(values):
    x, y, z, w = values
    return [-x, -z, -y, w]


def fields(value):
    return [value[key] for key in ('x', 'y', 'z', 'w')]


def product(left, right):
    x, y, z, w = left
    a, b, c, d = right
    return [w*a + x*d + y*c - z*b, w*b - x*c + y*d + z*a,
            w*c + x*b - y*a + z*d, w*d - x*a - y*b - z*c]


def product_f32(left, right):
    x, y, z, w = map(f32, left)
    a, b, c, d = map(f32, right)
    return [f32(f32(f32(w*a)+f32(x*d))+f32(f32(y*c)-f32(z*b))),
            f32(f32(f32(w*b)-f32(x*c))+f32(f32(y*d)+f32(z*a))),
            f32(f32(f32(w*c)+f32(x*b))+f32(f32(-y*a)+f32(z*d))),
            f32(f32(f32(w*d)-f32(x*a))+f32(f32(-y*b)-f32(z*c)))]


def main():
    root = Path('docs/audit/bf144/d262a')
    source = json.loads((root / 'rotation.native.json').read_text())['geometry_query_probe']['samples']
    rows = []
    for sample in source:
        request = sample['input']
        result = sample['foot_lock_rotation']
        lock = runtime(request['rotation_xyzw'])
        target = runtime(request['foot_lock_rotation']['target_rotation_xyzw'])
        inverse = [-target[0], -target[1], -target[2], target[3]]
        offset = runtime(fields(result['offset_rotation_float']))
        relative_native = list(map(f32, runtime(fields(result['relative_rotation']))))
        final_native = list(map(f32, runtime(fields(result['final_rotation']))))
        relative_wide = list(map(f32, product(lock, inverse)))
        final_wide = list(map(f32, product(offset, lock)))
        rows.append({'name': request['name'], 'limited': result['limited'],
                     'relative_native_f32': relative_native, 'relative_wide_f32': relative_wide,
                     'relative_old_f32': product_f32(lock, inverse),
                     'final_native_f32': final_native, 'final_wide_f32': final_wide,
                     'final_old_f32': product_f32(offset, lock)})
    report = {'rows': rows,
              'wide_relative_matches': sum(row['relative_native_f32'] == row['relative_wide_f32'] for row in rows),
              'wide_final_matches': sum(row['final_native_f32'] == row['final_wide_f32'] for row in rows),
              'old_relative_changes': sum(row['relative_native_f32'] != row['relative_old_f32'] for row in rows),
              'old_final_changes': sum(row['final_native_f32'] != row['final_old_f32'] for row in rows)}
    (root / 'products.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f"Compared {len(rows)} source quaternion pairs", flush=True)


if __name__ == '__main__':
    main()
