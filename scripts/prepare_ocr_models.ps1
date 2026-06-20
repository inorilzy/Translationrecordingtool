param(
    [ValidateSet("lite", "standard", "accurate")]
    [string]$Profile = "standard",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$modelRoot = Join-Path $repoRoot "src-tauri\resources\ocr-models"
$profileRoot = Join-Path $modelRoot $Profile
$cacheRoot = Join-Path $repoRoot ".ocr-model-cache"

$profiles = @{
    lite = @{
        det = "https://paddleocr.bj.bcebos.com/PP-OCRv3/chinese/ch_PP-OCRv3_det_infer.tar"
        rec = "https://paddleocr.bj.bcebos.com/PP-OCRv3/chinese/ch_PP-OCRv3_rec_infer.tar"
        cls = "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/ch_ppocr_mobile_v2.0_cls_infer.tar"
    }
    standard = @{
        det = "https://paddleocr.bj.bcebos.com/PP-OCRv4/chinese/ch_PP-OCRv4_det_infer.tar"
        rec = "https://paddleocr.bj.bcebos.com/PP-OCRv4/chinese/ch_PP-OCRv4_rec_infer.tar"
        cls = "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/ch_ppocr_mobile_v2.0_cls_infer.tar"
    }
    accurate = @{
        det = "https://paddleocr.bj.bcebos.com/PP-OCRv4/chinese/ch_PP-OCRv4_det_server_infer.tar"
        rec = "https://paddleocr.bj.bcebos.com/PP-OCRv4/chinese/ch_PP-OCRv4_rec_server_infer.tar"
        cls = "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/ch_ppocr_mobile_v2.0_cls_infer.tar"
    }
}

function Test-ModelDirReady {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        return $false
    }

    return [bool](Get-ChildItem -LiteralPath $Path -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -ne ".gitkeep" } |
        Select-Object -First 1)
}

function Expand-ModelArchive {
    param(
        [string]$ArchivePath,
        [string]$TargetDir
    )

    $extractRoot = Join-Path $cacheRoot ([System.IO.Path]::GetFileNameWithoutExtension($ArchivePath))
    if (Test-Path $extractRoot) {
        Remove-Item -LiteralPath $extractRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $extractRoot | Out-Null

    tar -xf $ArchivePath -C $extractRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to extract model archive: $ArchivePath"
    }

    $sourceDir = Get-ChildItem -LiteralPath $extractRoot -Directory | Select-Object -First 1
    if (-not $sourceDir) {
        throw "Model archive did not contain a model directory: $ArchivePath"
    }

    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
    Get-ChildItem -LiteralPath $TargetDir -Force |
        Where-Object { $_.Name -ne ".gitkeep" } |
        Remove-Item -Recurse -Force
    Get-ChildItem -LiteralPath $sourceDir.FullName -Force |
        Where-Object { -not $_.Name.StartsWith("._") } |
        Copy-Item -Destination $TargetDir -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $cacheRoot, $profileRoot | Out-Null

foreach ($kind in @("det", "rec", "cls")) {
    $targetDir = Join-Path $profileRoot $kind
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

    if ((Test-ModelDirReady $targetDir) -and (-not $Force)) {
        Write-Host "[ocr-models] $Profile/$kind already exists, skip. Use -Force to refresh."
        continue
    }

    $url = $profiles[$Profile][$kind]
    $archivePath = Join-Path $cacheRoot ([System.IO.Path]::GetFileName($url))

    if ((-not (Test-Path $archivePath)) -or $Force) {
        Write-Host "[ocr-models] Downloading $Profile/$kind from $url"
        Invoke-WebRequest -Uri $url -OutFile $archivePath
    } else {
        Write-Host "[ocr-models] Using cached archive: $archivePath"
    }

    Write-Host "[ocr-models] Extracting $Profile/$kind to $targetDir"
    Expand-ModelArchive -ArchivePath $archivePath -TargetDir $targetDir
}

Write-Host "[ocr-models] Done: $profileRoot"
