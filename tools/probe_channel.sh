#!/bin/bash
# 导出通道探测：1) execute_tool_script 环境说明 2) EditorAppToolset 工具清单
set -u
cd "$(dirname "$0")/.."

echo "== get_execution_environment =="
python tools/mcp_probe.py call editor_toolset.toolsets.programmatic.ProgrammaticToolset \
  get_execution_environment '{}' > tmp/als_enum/exec_env.json 2>&1 || true
python - <<'EOF'
import json
d = json.load(open('tmp/als_enum/exec_env.json', encoding='utf-8'))
env = d.get('returnValue') or {}
print('language:', env.get('language'))
print('supported_modules:')
for m in env.get('supported_modules') or []:
    print(' -', m.get('name'), '|', (m.get('description') or '')[:120])
print('--- instructions ---')
print((env.get('instructions') or '')[:3000])
EOF

echo
echo "== EditorAppToolset tools =="
python tools/mcp_probe.py call - describe_toolset \
  '{"toolset_name":"EditorToolset.EditorAppToolset"}' 2>/dev/null | \
python -c "
import json,sys
try:
    d=json.load(sys.stdin)
    for t in d['tools']:
        print(' ', t['name'].split('.')[-1])
except Exception as e:
    print('fail:', e)
"
