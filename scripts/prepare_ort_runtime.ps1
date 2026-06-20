param(
    [string]$Version = "1.20.1",
    [string]$Arch = "x64"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$binariesDir = Join-Path $repoRoot "src-tauri\binaries"
New-Item -ItemType Directory -Force $binariesDir | Out-Null

$dllPath = Join-Path $binariesDir "onnxruntime.dll"
if (Test-Path $dllPath) {
    Write-Host "ONNX Runtime DLL already exists: $dllPath"
    exit 0
}

if ($Arch -ne "x64") {
    throw "Only Windows x64 CPU ONNX Runtime is configured."
}

$packageName = "onnxruntime-win-x64-$Version"
$zipPath = Join-Path $binariesDir "$packageName.zip"
$extractDir = Join-Path $binariesDir $packageName
$url = "https://github.com/microsoft/onnxruntime/releases/download/v$Version/$packageName.zip"

Write-Host "Downloading ONNX Runtime $Version from $url"
Invoke-WebRequest -Uri $url -OutFile $zipPath

if (Test-Path $extractDir) {
    Remove-Item -LiteralPath $extractDir -Recurse -Force
}
Expand-Archive -LiteralPath $zipPath -DestinationPath $binariesDir -Force

$sourceDll = Join-Path $extractDir "lib\onnxruntime.dll"
if (!(Test-Path $sourceDll)) {
    throw "Downloaded package does not contain lib\onnxruntime.dll"
}

Copy-Item -LiteralPath $sourceDll -Destination $dllPath -Force
Remove-Item -LiteralPath $zipPath -Force
Remove-Item -LiteralPath $extractDir -Recurse -Force

Write-Host "ONNX Runtime DLL ready: $dllPath"
