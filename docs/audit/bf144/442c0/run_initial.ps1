$ErrorActionPreference = "Stop"
& ./docs/audit/bf144/442c0/run.ps1 -Label baseline
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./docs/audit/bf144/442c0/run.ps1 -Label invalid -Request invalid.request.json
if ($LASTEXITCODE -ne 7) { throw "Invalid player-view flag was accepted" }
exit 0
