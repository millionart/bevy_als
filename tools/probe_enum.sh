#!/bin/bash
# M0.5 枚举：ALS 工程内待导出资产清单 + 导出能力探测
# 用法：bash tools/probe_enum.sh （输出落 tmp/als_enum/，该目录已 gitignore）
set -u
cd "$(dirname "$0")/.."
mkdir -p tmp/als_enum

echo "-- folders(/Game recursive) --"
python tools/mcp_probe.py call editor_toolset.toolsets.asset.AssetTools list_folders \
  '{"root_path":"/Game","recursive":true}' > tmp/als_enum/folders.json 2>&1 || true
head -c 1500 tmp/als_enum/folders.json; echo

for cls in SkeletalMesh Skeleton AnimSequence AnimMontage BlendSpace BlendSpace1D BlendSpace2D CurveFloat CurveTable; do
  echo "-- $cls --"
  python tools/mcp_probe.py call editor_toolset.toolsets.asset.AssetTools find_assets \
    "{\"folder_path\":\"/Game\",\"name\":\"\",\"recursive\":true,\"asset_type\":{\"refPath\":\"/Script/Engine.$cls\"}}" \
    > "tmp/als_enum/$cls.json" 2>&1 || true
  python - "$cls" <<'EOF'
import json, sys
cls = sys.argv[1]
try:
    d = json.load(open(f'tmp/als_enum/{cls}.json', encoding='utf-8'))
    items = d.get('returnValue') or []
    print(len(items), 'assets')
    for p in items[:10]:
        print(' ', p)
    if len(items) > 10:
        print('  ...')
except Exception as e:
    txt = open(f'tmp/als_enum/{cls}.json', encoding='utf-8', errors='replace').read()[:200]
    print('parse fail:', e, '| raw:', txt)
EOF
done

echo "-- ProgrammaticToolset tools --"
python tools/mcp_probe.py call - describe_toolset \
  '{"toolset_name":"editor_toolset.toolsets.programmatic.ProgrammaticToolset"}' 2>/dev/null | \
  python -c "
import json,sys
try:
    d=json.load(sys.stdin)
    for t in d['tools']:
        print(' ', t['name'].split('.')[-1])
except Exception as e:
    print('fail:', e)
"
