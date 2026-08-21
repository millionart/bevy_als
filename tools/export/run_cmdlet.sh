#!/bin/bash
# 头less commandlet 启动器（路径经 tools/env.local.json 注入，禁止硬编码）
# 用法: bash tools/export/run_cmdlet.sh <script.py> [附加 UE 参数...]
set -u
cd "$(dirname "$0")/../.."

ENV_JSON="tools/env.local.json"
if [ ! -f "$ENV_JSON" ]; then
  echo "缺少 $ENV_JSON（复制 tools/env.example.json 并填入本机路径）" >&2
  exit 2
fi
UE=$(python -c "import json;print(json.load(open('$ENV_JSON'))['ue_cmd'])")
PROJ=$(python -c "import json;print(json.load(open('$ENV_JSON'))['ue_project'])")

SCRIPT="$(pwd)/$1"; shift
mkdir -p tmp/export
LOG="tmp/export/cmdlet_$(date +%s).log"
export BEVY_ALS_ROOT="$(pwd)"
"$UE" "$PROJ" -run=pythonscript -script="$SCRIPT" -stdout -ForceLogFlush -abslog="$(pwd)/$LOG" "$@"
ec=$?
echo "== exit=$ec log=$LOG =="
grep -E "M0EXPORT|PYEXPORT|Error:|Fatal|LogPython" "$LOG" | tail -30
exit $ec
