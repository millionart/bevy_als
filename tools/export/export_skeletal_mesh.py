import json
import os
from pathlib import Path

import unreal


def main():
    request = json.loads(Path(os.environ['BEVY_ALS_GLTF_EXPORT_REQUEST']).read_text(encoding='utf-8-sig'))
    output = Path(request['output'])
    report = Path(request['report'])
    if output.parent.exists() and any(output.parent.iterdir()):
        raise RuntimeError('Source export directory must be empty')
    result = {'asset': request['asset'], 'output': output.name, 'success': False, 'stage': 'load'}
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
        unreal.log_warning('SKELETAL_EXPORT load')
        asset = unreal.load_asset(request['asset'])
        if not isinstance(asset, unreal.SkeletalMesh):
            raise RuntimeError('Source asset is not a skeletal mesh')
        options = unreal.GLTFExportOptions()
        options.export_uniform_scale = 0.01
        options.export_vertex_skin_weights = True
        options.skip_near_default_values = False
        task = unreal.AssetExportTask()
        task.object = asset
        task.exporter = unreal.GLTFSkeletalMeshExporter()
        task.options = options
        task.filename = str(output)
        task.automated = True
        task.prompt = False
        result['stage'] = 'export'
        unreal.log_warning('SKELETAL_EXPORT export')
        if not unreal.Exporter.run_asset_export_task(task) or not output.is_file():
            raise RuntimeError('Skeletal mesh export failed')
        result.update(success=True, stage='complete', scale=0.01, skin_weights=True,
            files=sorted(path.name for path in output.parent.iterdir() if path.is_file()))
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
        unreal.log_warning('SKELETAL_EXPORT ' + json.dumps(result))


if __name__ == '__main__':
    main()
