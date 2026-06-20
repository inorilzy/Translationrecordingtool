param(
    [ValidateSet("tiny", "small", "medium", "lite", "standard", "accurate")]
    [string]$Profile = "small",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$modelRoot = Join-Path $repoRoot "src-tauri\resources\ocr-models"
$cacheRoot = Join-Path $repoRoot ".ocr-model-cache"

$profileAliases = @{
    lite = "tiny"
    standard = "small"
    accurate = "medium"
}

if ($profileAliases.ContainsKey($Profile)) {
    $Profile = $profileAliases[$Profile]
}

$profileRoot = Join-Path $modelRoot $Profile
$files = @("inference.onnx", "inference.json", "inference.yml")
$profiles = @{
    tiny = @{
        det = "PaddlePaddle/PP-OCRv6_tiny_det_onnx"
        rec = "PaddlePaddle/PP-OCRv6_tiny_rec_onnx"
    }
    small = @{
        det = "PaddlePaddle/PP-OCRv6_small_det_onnx"
        rec = "PaddlePaddle/PP-OCRv6_small_rec_onnx"
    }
    medium = @{
        det = "PaddlePaddle/PP-OCRv6_medium_det_onnx"
        rec = "PaddlePaddle/PP-OCRv6_medium_rec_onnx"
    }
}

function Test-ModelDirReady {
    param([string]$Path)

    return (Test-Path (Join-Path $Path "inference.onnx"))
}

function Download-HuggingFaceFile {
    param(
        [string]$Repo,
        [string]$FileName,
        [string]$TargetPath
    )

    $encodedRepo = $Repo -replace "/", "/"
    $url = "https://huggingface.co/$encodedRepo/resolve/main/$FileName"
    $cacheDir = Join-Path $cacheRoot ($Repo -replace "[/\\:]", "_")
    $cachePath = Join-Path $cacheDir $FileName

    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null

    if ((-not (Test-Path $cachePath)) -or $Force) {
        Write-Host "[ocr-models] Downloading $Repo/$FileName"
        Invoke-WebRequest -Uri $url -OutFile $cachePath
    } else {
        Write-Host "[ocr-models] Using cached file: $cachePath"
    }

    Copy-Item -Force -LiteralPath $cachePath -Destination $TargetPath
}

New-Item -ItemType Directory -Force -Path $cacheRoot, $profileRoot | Out-Null

foreach ($kind in @("det", "rec")) {
    $targetDir = Join-Path $profileRoot $kind
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

    if ((Test-ModelDirReady $targetDir) -and (-not $Force)) {
        Write-Host "[ocr-models] PP-OCRv6 $Profile/$kind already exists, skip. Use -Force to refresh."
        continue
    }

    Get-ChildItem -LiteralPath $targetDir -Force |
        Where-Object { $_.Name -ne ".gitkeep" } |
        Remove-Item -Recurse -Force

    $repo = $profiles[$Profile][$kind]
    foreach ($file in $files) {
        $targetPath = Join-Path $targetDir $file
        Download-HuggingFaceFile -Repo $repo -FileName $file -TargetPath $targetPath
    }
}

Write-Host "[ocr-models] Done: $profileRoot"
