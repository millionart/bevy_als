import argparse
import gzip
import json
from pathlib import Path

from import_movement_constants import NUMBER, literal


def capacity(facts, report):
    text = '\n'.join(item['text'] for item in facts)
    requested = literal(text, rf'MaxContactsBufferSize\s*=\s*{NUMBER}\s*;', 'contact capacity')
    samples = [sample for sample in report['physics_contact_probe']['samples']
               if 'mesh_contact_buffer_capacity' in sample]
    if len(samples) != 1 or samples[0]['input']['mesh_contact_buffer_request'] != requested:
        raise ValueError('Source contact buffer request is missing or ambiguous')
    result = samples[0]['mesh_contact_buffer_capacity']
    if type(result) is not int or result < requested:
        raise ValueError('Source contact buffer capacity is invalid')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.native.resolve()):
        raise ValueError('Target output must differ from source')
    raw = args.native.read_bytes()
    if args.native.suffix == '.gz':
        raw = gzip.decompress(raw)
    value = capacity(json.loads(args.facts.read_text()), json.loads(raw))
    args.output.write_text(f'{value}_usize\n')


if __name__ == '__main__':
    main()
