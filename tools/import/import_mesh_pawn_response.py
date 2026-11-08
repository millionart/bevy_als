import argparse
import gzip
import json
from pathlib import Path


def field(fields, name):
    matches = [value for value in fields if value['name'] == name]
    if len(matches) != 1:
        raise ValueError(f'Expected one {name} field')
    return matches[0]


def pawn_ignored(actor):
    meshes = [item for item in actor['blueprint_cdo_subobjects']
              if item['object_path'].endswith(':CharacterMesh0')]
    if len(meshes) != 1:
        raise ValueError('Expected one character mesh')
    body = field(meshes[0]['properties'], 'BodyInstance')
    responses = field(body['fields'], 'CollisionResponses')
    channels = field(responses['fields'], 'ResponseToChannels')
    response = field(channels['fields'], 'Pawn')['exported_text']
    if response not in ('ECR_Ignore', 'ECR_Block'):
        raise ValueError(f'Unsupported Pawn response: {response}')
    return response == 'ECR_Ignore'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--actor', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.actor.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    opener = gzip.open if args.actor.suffix == '.gz' else open
    with opener(args.actor, 'rt', encoding='utf-8-sig') as source:
        ignored = pawn_ignored(json.load(source))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'{str(ignored).lower()}\n', encoding='utf-8')


if __name__ == '__main__':
    main()
