import argparse
import re
from pathlib import Path
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE


def extract(math_header):
    values = re.findall(r'^#define\s+UE_SMALL_NUMBER\s+\(([\deE.+-]+)f\)', math_header, re.MULTILINE)
    if len(values) != 1:
        raise ValueError('Source input tolerance is missing or ambiguous')
    return float(values[0])


def generate(text, value, physical=False, acceleration=False):
    name = 'MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER' if acceleration else 'PHYSICAL_VECTOR_MIN_LENGTH_SQUARED' if physical else 'INPUT_DIRECTION_MIN_LENGTH_SQUARED'
    if physical:
        value *= SOURCE_LENGTH_TO_RUNTIME_SCALE ** 2
    elif acceleration:
        value *= SOURCE_LENGTH_TO_RUNTIME_SCALE
    pattern = rf'const {name}: f32 = [^;]+;'
    if len(re.findall(pattern, text)) != 1:
        raise ValueError('Target input tolerance is missing or ambiguous')
    return re.sub(pattern, f'const {name}: f32 = {value!r};', text)


def self_test():
    source = '#define UE_SMALL_NUMBER (1.e-8f)\n'
    target = 'const INPUT_DIRECTION_MIN_LENGTH_SQUARED: f32 = 0.0;\n'
    assert generate(target, extract(source)) == 'const INPUT_DIRECTION_MIN_LENGTH_SQUARED: f32 = 1e-08;\n'
    physical = 'const PHYSICAL_VECTOR_MIN_LENGTH_SQUARED: f32 = 0.0;\n'
    assert generate(physical, extract(source), True) == 'const PHYSICAL_VECTOR_MIN_LENGTH_SQUARED: f32 = 1e-12;\n'
    scalar = 'const MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER: f32 = 0.0;\n'
    assert generate(scalar, extract(source), acceleration=True) == 'const MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER: f32 = 1e-10;\n'
    assert generate(target, extract(source.replace('1.e-8', '2.e-8'))) != generate(target, extract(source))
    for broken in ['', source + source]:
        try:
            extract(broken)
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid source accepted')
    for broken in ['', target + target]:
        try:
            generate(broken, 1e-8)
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid target accepted')
    print('Input constants self-test PASS')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--math-header', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check', action='store_true')
    units = parser.add_mutually_exclusive_group()
    units.add_argument('--physical', action='store_true')
    units.add_argument('--acceleration', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if not all([args.math_header, args.output]):
        parser.error('--math-header and --output are required')
    original = args.output.read_text(encoding='utf-8')
    generated = generate(original, extract(args.math_header.read_text(encoding='utf-8-sig')), args.physical, args.acceleration)
    if args.check:
        if original != generated:
            raise ValueError('Target input tolerance differs from source')
    else:
        args.output.write_text(generated, encoding='utf-8', newline='\n')
    print('Input constants source check PASS' if args.check else 'Input constants generated')


if __name__ == '__main__':
    main()
