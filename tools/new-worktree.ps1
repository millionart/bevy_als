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

Write-Host "[1/4] syncing tracked-file mtimes from $root"
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

Write-Host "[2/4] copying build cache (deps/build/fingerprints, excluding incremental)"
robocopy (Join-Path $root "target") (Join-Path $dest "target") /E /MT:16 `
    /XD incremental target_game tmp `
    /NFL /NDL /NJH /NJS /NP /DCOPY:DAT /COPY:DAT | Out-Null
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with $LASTEXITCODE" }
Write-Host "      robocopy done (code $LASTEXITCODE)"

$gameCache = Join-Path $root "target_game"
if (Test-Path -LiteralPath $gameCache) {
    Write-Host "[3/4] copying manual playground cache (excluding incremental)"
    robocopy $gameCache (Join-Path $dest "target_game") /E /MT:16 `
        /XD incremental tmp `
        /NFL /NDL /NJH /NJS /NP /DCOPY:DAT /COPY:DAT | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy failed with $LASTEXITCODE" }
    Write-Host "      robocopy done (code $LASTEXITCODE)"
} else {
    Write-Host "[3/4] manual playground cache not found; skipped"
}

Write-Host "[4/4] done. Worktree at $dest"
Write-Host "Build inside it uses its own isolated target (no lock contention)."
