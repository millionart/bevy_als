# Creates a git worktree with a pre-warmed build cache:
#   1. git worktree add
#   2. syncs tracked-file mtimes from this checkout (cargo fingerprints are mtime-based)
#   3. copies target build cache (deps/build/fingerprints; excludes incremental & game dir)
# Usage: .\tools\new-worktree.ps1 -Name bevy_als_wt2 [-Branch somebranch]
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$Branch
)

$ErrorActionPreference = "Stop"
$root = (git rev-parse --show-toplevel).Trim()
$dest = Join-Path (Split-Path $root) $Name

if (Test-Path $dest) { throw "destination exists: $dest" }
if ($Branch) {
    $exists = git branch --list $Branch
    if ($exists) {
        git worktree add $dest $Branch
    } else {
        git worktree add -b $Branch $dest
    }
} else {
    git worktree add $dest
}
if ($LASTEXITCODE -ne 0) { throw "git worktree add failed" }

Write-Host "[1/3] syncing tracked-file mtimes from $root"
$files = git -C $root ls-files
$synced = 0
foreach ($f in $files) {
    $src = Join-Path $root $f
    $dst = Join-Path $dest $f
    if ((Test-Path -LiteralPath $src) -and (Test-Path -LiteralPath $dst)) {
        $st = Get-Item -LiteralPath $src
        (Get-Item -LiteralPath $dst).LastWriteTimeUtc = $st.LastWriteTimeUtc
        $synced++
    }
}
Write-Host "      synced $synced files"

Write-Host "[2/3] copying build cache (deps/build/fingerprints, excluding incremental)"
robocopy (Join-Path $root "target") (Join-Path $dest "target") /E /MT:16 `
    /XD incremental target_game tmp `
    /NFL /NDL /NJH /NJS /NP /DCOPY:DAT /COPY:DAT | Out-Null
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with $LASTEXITCODE" }
Write-Host "      robocopy done (code $LASTEXITCODE)"

Write-Host "[3/3] done. Worktree at $dest"
Write-Host "Build inside it uses its own isolated target (no lock contention)."
