#!/bin/bash
# 无头 commandlet 运行器
# 用法: bash tools/export/run_cmdlet.sh <script.py> [额外 UE 参数...]
set -u
cd "$(dirname "$0")/../.."
UE="<ue-5.8-install>/Engine/Binaries/Win64/UnrealEditor-Cmd.exe"
PROJ="<als-project>/ALSRefactored.uproject"
SCRIPT="$(pwd)/$1"; shift
mkdir -p tmp/export
LOG="tmp/export/cmdlet_$(date +%s).log"
"$UE" "$PROJ" -run=pythonscript -script="$SCRIPT" -stdout -ForceLogFlush -abslog="$(pwd)/$LOG" "$@"
ec=$?
echo "== exit=$ec log=$LOG =="
grep -E "PYPROBE|PYEXPORT|Error:|Fatal|LogPython" "$LOG" | tail -30
exit $ec
