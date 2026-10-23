# 准备 Windows ONNX Runtime DLL（src-tauri/binaries/onnxruntime.dll）。
#
# 默认固定官方 1.20.1 / win / x64：检查 zip 已知大小、唯一内部路径、DLL 的 PE 与文件版本。
# 默认没有上游摘要，不声称密码学来源验证，也不要求 release-assets.json 或人工审批。
# 显式 -ManifestPath 是自愿严格模式：保留清单选择、zip / DLL 大小与 SHA-256 校验。
# 其他版本 / 架构必须显式提供清单。下载与校验均在同卷暂存目录完成；替换后复验失败回滚。
#
# -ManifestPath / -BinariesDir 可重定向输入与输出；所有生产调用均拒绝 synthetic 清单。
# 离线测试通过限定作用域的 mock loader 加载合成数据，不提供生产放行开关。
[CmdletBinding()]
param(
    [string]$Version = "1.20.1",
    [string]$Arch = "x64",
    [string]$ManifestPath,
    [string]$BinariesDir
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot "AssetManifest.psm1") -Force

$platform = "win"
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $BinariesDir) { $BinariesDir = Join-Path $repoRoot "src-tauri\binaries" }

# 先确定计划，再做文件系统修改。默认路径不读取任何清单。
$strictManifest = -not [string]::IsNullOrWhiteSpace($ManifestPath)
$packageName = "onnxruntime-$platform-$Arch-$Version"
if ($strictManifest) {
    $manifest = Import-AssetManifest -Path $ManifestPath
    $asset = Find-ApprovedAsset -Manifest $manifest -Kind "ort-runtime" -Selector @{
        version = $Version; platform = $platform; arch = $Arch
    }
    $zipEntry = Get-AssetManifestFile -Asset $asset -Name "$packageName.zip"
    $dllEntry = Get-AssetManifestFile -Asset $asset -Name "onnxruntime.dll"
} else {
    if ($Version -cne '1.20.1' -or $Arch -cne 'x64') {
        throw 'Only 1.20.1/win/x64 is supported by default; overrides require explicit -ManifestPath.'
    }
    $asset = @{ id = 'ort-runtime/1.20.1/win-x64 (size/PE/version checks; no cryptographic verification)' }
    $zipEntry = @{
        name = 'onnxruntime-win-x64-1.20.1.zip'
        url = 'https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip'
        size = [long]65585496
    }
    # 无可信 DLL 大小；256 MiB 是解压上限而非声称的资产大小。
    $dllEntry = @{ archivePath = 'onnxruntime-win-x64-1.20.1/lib/onnxruntime.dll'; size = [long]268435456 }
}

function Test-OrtDll {
    param([string]$Path)
    if ($strictManifest) { return Test-AssetFile -Path $Path -FileEntry $dllEntry }
    # 只读取 PE 头和 FileVersionInfo，不加载或执行 DLL。
    $stream = [IO.File]::OpenRead($Path)
    $reader = New-Object IO.BinaryReader($stream)
    try {
        if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { return $false }
        $stream.Position = 0x3c
        $peOffset = $reader.ReadUInt32()
        if ($peOffset -lt 64 -or $peOffset -gt ($stream.Length - 24)) { return $false }
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0x8664) { return $false }
        $sections = $reader.ReadUInt16()
        $stream.Position = $peOffset + 20
        $optionalSize = $reader.ReadUInt16()
        $characteristics = $reader.ReadUInt16()
        if (($characteristics -band 0x2002) -ne 0x2002 -or $sections -eq 0 -or $optionalSize -lt 112) { return $false }
        if (($peOffset + 24 + $optionalSize + 40 * $sections) -gt $stream.Length) { return $false }
        if ($reader.ReadUInt16() -ne 0x20b) { return $false }
        for ($i = 0; $i -lt $sections; $i++) {
            $stream.Position = $peOffset + 24 + $optionalSize + 40 * $i + 16
            $rawSize = $reader.ReadUInt32()
            $rawOffset = $reader.ReadUInt32()
            if (([long]$rawOffset + $rawSize) -gt $stream.Length) { return $false }
        }
    } finally { $reader.Dispose(); $stream.Dispose() }
    # FileInfo.VersionInfo 由 System.Diagnostics.FileVersionInfo 读取；Get-Item 是离线测试 seam。
    # 官方 1.20.1 包的 DLL 文件版本是 1.20.<日期>.<内部号>，因此只校验主/次版本，不猜 build 段。
    $info = (Get-Item -LiteralPath $Path).VersionInfo
    return ($info.FileMajorPart -eq 1 -and $info.FileMinorPart -eq 20)
}

New-Item -ItemType Directory -Force $BinariesDir | Out-Null
$dllPath = Join-Path $BinariesDir "onnxruntime.dll"

if (Test-Path -LiteralPath $dllPath -PathType Leaf) {
    if (Test-OrtDll -Path $dllPath) {
        Write-Host "ONNX Runtime DLL already verified ($($asset['id'])): $dllPath"
        return
    }
    Write-Warning "Existing $dllPath does not match $($asset['id']); it will be replaced only after download validation."
}

# 只解出计划指定的唯一条目（区分大小写、精确路径）；
# 读取量超过清单大小或默认解压上限即中止，避免异常条目写满磁盘。
function Expand-VerifiedArchiveEntry {
    param([string]$ZipPath, [string]$EntryPath, [string]$Destination, [long]$ExpectedSize)
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $found = @($zip.Entries | Where-Object { $_.FullName -ceq $EntryPath })
        if ($found.Count -eq 0) { throw "Archive does not contain $EntryPath" }
        if ($found.Count -gt 1) { throw "Archive contains duplicate entries for $EntryPath" }
        $source = $found[0].Open()
        try {
            $target = [System.IO.File]::Open($Destination, [System.IO.FileMode]::CreateNew)
            try {
                $buffer = New-Object byte[] 81920
                $total = [long]0
                while (($read = $source.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $total += $read
                    if ($total -gt $ExpectedSize) {
                        throw "Archive entry $EntryPath is larger than the approved size $ExpectedSize"
                    }
                    $target.Write($buffer, 0, $read)
                }
            } finally { $target.Dispose() }
        } finally { $source.Dispose() }
    } finally { $zip.Dispose() }
}

# 临时目录与目标同目录（同卷），最终替换为同卷 File.Replace / File.Move。
$stagingDir = Join-Path $BinariesDir (".ort-staging-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory $stagingDir | Out-Null
$preserveBackup = $false
try {
    $zipPath = Join-Path $stagingDir $zipEntry["name"]
    $stagedDll = Join-Path $stagingDir "onnxruntime.dll"
    $backupDll = Join-Path $stagingDir "onnxruntime.dll.previous"

    Write-Host "Downloading ONNX Runtime $Version from $($zipEntry['url'])"
    $previousProgress = $ProgressPreference
    $ProgressPreference = "SilentlyContinue"
    try {
        Invoke-WebRequest -Uri $zipEntry["url"] -OutFile $zipPath -UseBasicParsing
    } finally { $ProgressPreference = $previousProgress }

    if ($strictManifest) {
        Assert-AssetFile -Path $zipPath -FileEntry $zipEntry
    } elseif ((Get-Item -LiteralPath $zipPath).Length -ne $zipEntry['size']) {
        throw 'ONNX Runtime archive size mismatch'
    }
    Expand-VerifiedArchiveEntry -ZipPath $zipPath -EntryPath $dllEntry["archivePath"] `
        -Destination $stagedDll -ExpectedSize ([long]$dllEntry["size"])
    if ($strictManifest) {
        Assert-AssetFile -Path $stagedDll -FileEntry $dllEntry
    } elseif (-not (Test-OrtDll -Path $stagedDll)) {
        throw 'ONNX Runtime DLL PE/version mismatch (expected x64 DLL 1.20.1)'
    }

    if (Test-Path -LiteralPath $dllPath -PathType Leaf) {
        [System.IO.File]::Replace($stagedDll, $dllPath, $backupDll)
    } else {
        [System.IO.File]::Move($stagedDll, $dllPath)
    }

    # false 与读取/校验异常属于同一失败路径，必须恢复替换前的文件。
    try {
        if (-not (Test-OrtDll -Path $dllPath)) {
            throw "Installed $dllPath failed post-install verification for $($asset['id'])"
        }
    } catch {
        $verificationError = $_
        try {
            if (Test-Path -LiteralPath $backupDll -PathType Leaf) {
                [System.IO.File]::Copy($backupDll, $dllPath, $true)
            } else {
                Remove-Item -LiteralPath $dllPath -Force
            }
        } catch {
            # 恢复失败时不清理暂存目录，避免删除唯一旧 DLL；错误明确给出恢复位置。
            $preserveBackup = $true
            throw "Post-install verification failed: $($verificationError.Exception.Message). Recovery failed: $($_.Exception.Message). Recovery backup retained at $backupDll"
        }
        throw $verificationError
    }
} finally {
    if (-not $preserveBackup) {
        Remove-Item -LiteralPath $stagingDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "ONNX Runtime DLL ready and verified ($($asset['id'])): $dllPath"
