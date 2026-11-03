$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/491"
$reader = "tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp"
$package = "target/ue_graph_source_reader_package"
function Check-Package([string]$Label) {
    $source = (Get-FileHash $reader -Algorithm SHA256).Hash.ToLower()
    $copy = (Get-FileHash $reader.Replace("tools/ue_graph_source_reader/", "$package/") -Algorithm SHA256).Hash.ToLower()
    if ($source -ne $copy) { throw "Reader package and project source differ" }
    $binary = (Get-FileHash "$package/Binaries/Mac/libUnrealEditor-AlsGraphSourceReader.dylib" -Algorithm SHA256).Hash.ToLower()
    @{source_sha256=$source; package_source_sha256=$copy; package_binary_sha256=$binary} |
        ConvertTo-Json | Set-Content "$root/package-$Label.json"
}
function Invoke-Checked([string]$Script, [hashtable]$Arguments) {
    & "$root/$Script.ps1" @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Script failed" }
}
Check-Package "baseline"
try {
    foreach ($mode in @("no-initial-write","no-frame-write")) {
        Invoke-Checked "run_python" @{Name="ablate"; Case=$mode}
        Invoke-Checked "build" @{Label="build-$mode"}
        Check-Package $mode
        Invoke-Checked "run" @{Label=$mode; Request="switch"}
    }
} finally {
    Invoke-Checked "run_python" @{Name="ablate"; Case="restore"}
    Invoke-Checked "build" @{Label="build-restored"}
    Check-Package "restored"
}
Invoke-Checked "run" @{Label="switch-restored"; Request="switch"}
Invoke-Checked "run" @{Label="legacy-after"; Request="legacy"}
Invoke-Checked "run" @{Label="invalid-rejected"; Request="invalid"; ExpectedExit=7}
