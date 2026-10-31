param([string]$Runner = "tools/run_animation_visual_validation.ps1", [string]$Label = "baseline")
$ErrorActionPreference = "Stop"
$workspaceRoot = (Get-Location).Path
$source = Get-Content -Raw -LiteralPath $Runner
$start = $source.IndexOf('$evidenceRoot =')
$end = $source.IndexOf('$variables = @(')
if ($start -lt 0 -or $end -le $start) { throw "runner routing section is missing" }
$routing = $source.Substring($start, $end - $start)
function Invoke-Routing {
    param([string]$Requested)
    $OutputDirectory = $Requested
    $Case = "startup"
    Invoke-Expression $routing
    return $candidate
}
$base = "docs/audit/bf144/441c0/fixtures/$Label"
if (Test-Path -LiteralPath $base) { throw "routing fixture already exists" }
$existing = Join-Path $base "existing"
$null = New-Item -ItemType Directory -Force -Path $existing
$sentinel = Join-Path $existing "sentinel.txt"
[IO.File]::WriteAllText($sentinel, "keep original evidence")
$fresh = Join-Path $base "fresh"
$result = Invoke-Routing $fresh
if (-not (Test-Path -LiteralPath $result -PathType Container)) { throw "Git evidence directory was not created" }
$candidate = $result
$environmentLine = ($source -split "`n" | Where-Object { $_ -match '^    \$env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR = ' })
if (@($environmentLine).Count -ne 1) { throw "evidence environment assignment is ambiguous" }
Invoke-Expression $environmentLine
if ([IO.Path]::IsPathRooted($env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR) -or [IO.Path]::GetFullPath((Join-Path $workspaceRoot $env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR)) -ne $candidate) { throw "evidence environment path is not relative" }
$reportPath = Join-Path $candidate "report.json"
$reportExpression = [regex]::Match($source, '(?m)^            report_path = (.+)$').Groups[1].Value
if (-not $reportExpression) { throw "runner report path expression is missing" }
$reported = Invoke-Expression $reportExpression
if ([IO.Path]::IsPathRooted($reported) -or [IO.Path]::GetFullPath((Join-Path $workspaceRoot $reported)) -ne $reportPath) { throw "runner report path is not relative" }
$outside = "target/animation_visual/routing-$Label"
$rejected = $false
try { $null = Invoke-Routing $outside } catch { $rejected = $true }
if (-not $rejected -or (Test-Path -LiteralPath $outside)) { throw "build-directory evidence was accepted" }
$rejected = $false
try { $null = Invoke-Routing $existing } catch { $rejected = $true }
if (-not $rejected -or [IO.File]::ReadAllText($sentinel) -ne "keep original evidence") { throw "existing evidence was overwritten" }
$report = @{ gate = "PASS_ACTUAL_VISUAL_RUNNER_ROUTING_WITHOUT_GAME_START"; git_directory_accepted = $true; build_directory_rejected = $true; existing_evidence_preserved = $true; game_started = $false; environment_and_report_paths_relative = $true }
$report | ConvertTo-Json | Set-Content -LiteralPath "docs/audit/bf144/441c0/$Label.json"
$report | ConvertTo-Json -Compress
