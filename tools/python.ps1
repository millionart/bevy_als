$candidate = Join-Path (Split-Path $PSScriptRoot) $(if ($IsWindows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" })
if (Test-Path -LiteralPath $candidate -PathType Leaf) {
    $candidate
} elseif ($IsWindows) {
    "python"
} else {
    "python3"
}
