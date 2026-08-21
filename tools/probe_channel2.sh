#!/bin/bash
# 导出通道候选探测：ObjectTools / CurveTableTools / SequencerImportExportTools
set -u
cd "$(dirname "$0")/.."

for ts in \
  "editor_toolset.toolsets.object.ObjectTools" \
  "editor_toolset.toolsets.curve_table.CurveTableTools" \
  "animation_toolset.toolsets.import_export.SequencerImportExportTools"; do
  echo "== $ts =="
  python tools/mcp_probe.py call - describe_toolset "{\"toolset_name\":\"$ts\"}" 2>/dev/null | \
  python -c "
import json,sys
try:
    d=json.load(sys.stdin)
    for t in d['tools']:
        args=','.join(t.get('inputSchema',{}).get('properties',{}).keys())
        print(' ', t['name'].split('.')[-1], '(', args, ')')
except Exception as e:
    print('fail:', e)
"
done
