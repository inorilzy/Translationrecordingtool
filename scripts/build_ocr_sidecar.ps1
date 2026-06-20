param(
    [string]$PythonVersion = "3.11",
    [string]$PaddleOcrVersion = "2.7.3",
    [string]$PaddlePaddleVersion = "2.6.2",
    [string]$RapidOcrVersion = "1.4.4",
    [string]$OnnxRuntimeVersion = "1.16.3"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$serverScript = Join-Path $repoRoot "scripts\paddle_ocr_server.py"
$buildRoot = Join-Path $repoRoot ".ocr-sidecar-build"
$venvPath = Join-Path $buildRoot ".venv"
$distPath = Join-Path $buildRoot "dist"
$workPath = Join-Path $buildRoot "build"
$binariesDir = Join-Path $repoRoot "src-tauri\binaries"
$targetName = "paddle-ocr-server-x86_64-pc-windows-msvc.exe"
$targetPath = Join-Path $binariesDir $targetName

if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    throw "uv was not found. Install uv first: https://docs.astral.sh/uv/getting-started/installation/"
}

if (-not (Test-Path $serverScript)) {
    throw "OCR server script was not found: $serverScript"
}

New-Item -ItemType Directory -Force -Path $buildRoot, $binariesDir | Out-Null

Write-Host "[ocr-sidecar] Preparing Python $PythonVersion environment..."
uv venv --clear --python $PythonVersion $venvPath
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create Python virtual environment"
}

$python = Join-Path $venvPath "Scripts\python.exe"
if (-not (Test-Path $python)) {
    throw "Python virtual environment was not created: $python"
}

Write-Host "[ocr-sidecar] Installing dependencies..."
uv pip install --python $python "paddleocr==$PaddleOcrVersion" "paddlepaddle==$PaddlePaddleVersion" "rapidocr-onnxruntime==$RapidOcrVersion" "onnxruntime==$OnnxRuntimeVersion" "numpy<2" "pyinstaller>=6,<7"
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install OCR sidecar dependencies"
}

$paddleOcrPackageDir = Join-Path $venvPath "Lib\site-packages\paddleocr"
if (-not (Test-Path $paddleOcrPackageDir)) {
    throw "Failed to locate paddleocr package directory"
}

$rapidOcrPackageDir = Join-Path $venvPath "Lib\site-packages\rapidocr_onnxruntime"
if (-not (Test-Path $rapidOcrPackageDir)) {
    throw "Failed to locate rapidocr_onnxruntime package directory"
}

Write-Host "[ocr-sidecar] Building executable..."
& $python -m PyInstaller `
    --noconfirm `
    --clean `
    --onefile `
    --name "paddle-ocr-server" `
    --specpath $buildRoot `
    --distpath $distPath `
    --workpath $workPath `
    --paths $paddleOcrPackageDir `
    --paths $rapidOcrPackageDir `
    --collect-all paddleocr `
    --collect-all rapidocr_onnxruntime `
    --collect-all onnxruntime `
    --collect-all paddle `
    --collect-all Cython `
    --collect-all pyclipper `
    --collect-all lmdb `
    --copy-metadata imageio `
    --copy-metadata imgaug `
    --copy-metadata scikit-image `
    --copy-metadata scipy `
    --copy-metadata shapely `
    --copy-metadata pyclipper `
    --copy-metadata lmdb `
    --copy-metadata opencv-python `
    --copy-metadata opencv-contrib-python `
    --copy-metadata paddleocr `
    --copy-metadata paddlepaddle `
    --copy-metadata rapidocr-onnxruntime `
    --copy-metadata onnxruntime `
    --hidden-import paddleocr `
    --hidden-import rapidocr_onnxruntime `
    --hidden-import onnxruntime `
    --hidden-import paddle `
    --hidden-import ppocr `
    --hidden-import ppstructure `
    --hidden-import tools `
    --hidden-import pyclipper `
    --hidden-import lmdb `
    $serverScript
if ($LASTEXITCODE -ne 0) {
    throw "Failed to build OCR sidecar with PyInstaller"
}

$builtExe = Join-Path $distPath "paddle-ocr-server.exe"
if (-not (Test-Path $builtExe)) {
    throw "PyInstaller did not create expected file: $builtExe"
}

Copy-Item -Force -LiteralPath $builtExe -Destination $targetPath
Write-Host "[ocr-sidecar] Done: $targetPath"
