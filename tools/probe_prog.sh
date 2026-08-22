#!/bin/bash
set -u
cd "$(dirname "$0")/.."
python tools/mcp_probe.py call - describe_toolset \
  '{"toolset_name":"editor_toolset.toolsets.programmatic.ProgrammaticToolset"}' 2>/dev/null | \
python -c "
import json,sys
d=json.load(sys.stdin)
print(json.dumps(d,indent=1)[:6000])
"
