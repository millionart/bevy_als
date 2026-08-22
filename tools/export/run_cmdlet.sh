#!/bin/bash
set -u
cd "$(dirname "$0")/../.."

ENV_JSON="tools/env.local.json"
if [ ! -f "$ENV_JSON" ]; then
  echo "missing $ENV_JSON (copy tools/env.example.json and fill in local paths)" >&2
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
