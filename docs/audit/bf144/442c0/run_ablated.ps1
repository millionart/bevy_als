$ErrorActionPreference = "Stop"
& ./docs/audit/bf144/442c0/run.ps1 -Label ablated
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./docs/audit/bf144/442c0/run.ps1 -Label ablated-invalid -Request invalid.request.json
if ($LASTEXITCODE -ne 0) { throw "Removed type guard did not accept the string flag" }
& ./docs/audit/bf144/442c0/run.ps1 -Label ablated-legacy -Request legacy-mini.request.json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
exit 0
