$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/522"
$source = "$root/contact.rs"
$original = [IO.File]::ReadAllBytes($source)
$flag = 'const C522_CLEAR_CONTACT: bool = false;'
$text = [Text.Encoding]::UTF8.GetString($original)
if (-not $text.Contains($flag)) { throw "Expected original contact observer" }
try {
    [IO.File]::WriteAllText($source, $text.Replace($flag, 'const C522_CLEAR_CONTACT: bool = true;'))
    & "$root/run_python.ps1" -Name observer -Case cleared
    if ($LASTEXITCODE) { throw "Control preparation failed" }
    & "$root/compile.ps1" -Label cleared
    if ($LASTEXITCODE) { throw "Control build failed" }
    & "$root/playground.ps1" -Label cleared -HalfWidthCm 100 -Frame 49 -WallCm 162
    if ($LASTEXITCODE) { throw "Control runtime failed" }
    & "$root/functional.ps1" -Label cleared
    if ($LASTEXITCODE) { throw "Control functional gate failed" }
} finally {
    [IO.File]::WriteAllBytes($source, $original)
}
& "$root/run_python.ps1" -Name observer -Case restored
if ($LASTEXITCODE) { throw "Restored preparation failed" }
& "$root/compile.ps1" -Label restored
if ($LASTEXITCODE) { throw "Restored build failed" }
& "$root/playground.ps1" -Label restored -HalfWidthCm 100 -Frame 49 -WallCm 162
if ($LASTEXITCODE) { throw "Restored runtime failed" }
& "$root/functional.ps1" -Label restored
if ($LASTEXITCODE) { throw "Restored functional gate failed" }
