param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$Branch
)

$ErrorActionPreference = "Stop"
$root = (git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0) { throw "git root lookup failed" }
$dest = Join-Path (Split-Path $root) $Name

if (Test-Path $dest) { throw "destination exists: $dest" }
if (-not $Branch) { $Branch = "codex/$Name" }
$exists = git branch --list $Branch
if ($exists) {
    git worktree add $dest $Branch
} else {
    git worktree add -b $Branch $dest
}
if ($LASTEXITCODE -ne 0) { throw "git worktree add failed" }

. (Join-Path $PSScriptRoot "run_ue_source_reader.ps1") -FunctionsOnly
$python = & (Join-Path $PSScriptRoot "python.ps1")
$copy = @'
from pathlib import Path
import os, shutil, subprocess, sys
source, destination = map(Path, sys.argv[1:])
for raw in subprocess.check_output(["git", "-C", str(source), "ls-files", "-z"]).split(b"\0"):
    if not raw:
        continue
    name = os.fsdecode(raw)
    original, copied = source / name, destination / name
    if original.is_file() and copied.is_file():
        stat = original.stat()
        os.utime(copied, ns=(stat.st_atime_ns, stat.st_mtime_ns))
        print(name, flush=True)
def copy_file(original, copied):
    result = shutil.copy2(original, copied)
    print(original, flush=True)
    return result
for folder in ["target", "target_game/play"]:
    if (source / folder).is_dir():
        excluded = ["incremental", "target_game", "tmp"]
        if folder == "target_game/play":
            excluded.append("*.pdb")
        shutil.copytree(source / folder, destination / folder, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns(*excluded), copy_function=copy_file)
config = source / "tools/env.local.json"
if config.is_file():
    (destination / "tools").mkdir(exist_ok=True)
    shutil.copy2(config, destination / "tools/env.local.json")
'@
$logs = Join-Path $PSScriptRoot ("tmp/worktrees/" + $Name)
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$result = Invoke-ProcessWithWatchdog -FilePath $python -ArgumentList @("-c", $copy, $root, $dest) -ReportPath (Join-Path $logs "copy.report.json") -StdoutPath (Join-Path $logs "copy.stdout.log") -StderrPath (Join-Path $logs "copy.stderr.log") -ProgressPaths @((Join-Path $logs "copy.stdout.log"), (Join-Path $logs "copy.stderr.log")) -TimeoutSeconds 3600 -NoProgressSeconds 120
if (-not $result.success) { throw "worktree cache copy failed: $($result.exit_reason); see $logs" }
Write-Output "Worktree ready at $dest with its own target and target_game."
