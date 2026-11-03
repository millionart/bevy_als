$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/441a"
$reader = "tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp"
$packageReader = $reader.Replace("tools/ue_graph_source_reader/", "target/ue_graph_source_reader_package/")
$packageBinary = "target/ue_graph_source_reader_package/Binaries/Mac/libUnrealEditor-AlsGraphSourceReader.dylib"
function Check-Package([string]$Label) {
    $source = (Get-FileHash $reader -Algorithm SHA256).Hash.ToLower()
    $copy = (Get-FileHash $packageReader -Algorithm SHA256).Hash.ToLower()
    if ($source -ne $copy) { throw "Package source differs from current reader" }
    $binary = (Get-FileHash $packageBinary -Algorithm SHA256).Hash.ToLower()
    @{source_sha256=$source; package_source_sha256=$copy; package_binary_sha256=$binary} |
        ConvertTo-Json | Set-Content "$root/package-$Label.json"
}
function Invoke-Case([string]$Label,[string]$Kind,[int]$ExpectedExit=0) {
    & "$root/run.ps1" -Label $Label -Kind $Kind -ExpectedExit $ExpectedExit
    if ($LASTEXITCODE -ne 0) { throw "$Label native probe failed" }
}
& "$root/run_python.ps1" -Name ablate -Case snapshot
if ($LASTEXITCODE -ne 0) { throw "Reader snapshot failed" }
Check-Package "baseline"
Invoke-Case "vector-baseline" "vector"
Invoke-Case "float-baseline" "float"
Invoke-Case "character-baseline" "character"
try {
    & "$root/run_python.ps1" -Name ablate -Case axis
    if ($LASTEXITCODE -ne 0) { throw "Axis ablation source failed" }
    & "$root/build.ps1" -Label build-axis
    if ($LASTEXITCODE -ne 0) { throw "Axis ablation build failed" }
    Check-Package "axis"
    Invoke-Case "vector-axis-ablated" "vector"
    & "$root/run_python.ps1" -Name ablate -Case guard
    if ($LASTEXITCODE -ne 0) { throw "Type ablation source failed" }
    & "$root/build.ps1" -Label build-guard
    if ($LASTEXITCODE -ne 0) { throw "Type ablation build failed" }
    Check-Package "guard"
    Invoke-Case "string-guard-ablated" "string"
} finally {
    & "$root/run_python.ps1" -Name ablate -Case restore
    if ($LASTEXITCODE -ne 0) { throw "Reader source restore failed" }
    & "$root/build.ps1" -Label build-restored
    if ($LASTEXITCODE -ne 0) { throw "Reader restore build failed" }
    Check-Package "restored"
}
Invoke-Case "vector-restored" "vector"
Invoke-Case "float-restored" "float"
Invoke-Case "character-restored" "character"
foreach ($kind in @("string","range","fraction")) { Invoke-Case "$kind-rejected" $kind 7 }
