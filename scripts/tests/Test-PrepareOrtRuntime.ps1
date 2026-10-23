# scripts/prepare_ort_runtime.ps1 的离线合成测试，无 Pester 依赖，不访问网络或用户缓存。
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/tests/Test-PrepareOrtRuntime.ps1
#   pwsh -NoProfile -File scripts/tests/Test-PrepareOrtRuntime.ps1
#
# Fixture 全部在系统临时目录内运行时生成并标注 SYNTHETIC：小 zip（含假 DLL 字节）和与之匹配的
# synthetic 清单（synthetic=true，仅在限定 mock loader 内加载）。这些 hash 只描述假文件，
# 不得写入 scripts/release-assets.json，也不能证明真实 ORT 包可信（见工单 06）。
#
# 网络隔离：在本测试作用域定义同名函数 Invoke-WebRequest（函数优先于 cmdlet，被调脚本动态继承），
# 只把本地 fixture 复制到 -OutFile，或模拟下载失败；生产子进程拒绝测试清单，下载不可达。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$scriptUnderTest = (Resolve-Path (Join-Path $PSScriptRoot '../prepare_ort_runtime.ps1')).ProviderPath
$modulePath = (Resolve-Path (Join-Path $PSScriptRoot '../AssetManifest.psm1')).ProviderPath
Import-Module $modulePath -Force
$version = '1.20.1'
$package = "onnxruntime-win-x64-$version"
$zipName = "$package.zip"
$archivePath = "$package/lib/onnxruntime.dll"
$zipUrl = "https://github.com/microsoft/onnxruntime/releases/download/v$version/$zipName"

$root = Join-Path ([IO.Path]::GetTempPath()) ('prepare-ort-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $root | Out-Null
$fixtures = Join-Path $root 'fixtures'
New-Item -ItemType Directory $fixtures | Out-Null

function Get-Sha256 { param([string]$Path) (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }

function New-FixtureZip {
    param([string]$Path, [hashtable]$Entries)
    $zip = [IO.Compression.ZipFile]::Open($Path, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $Entries.Keys) {
            $stream = $zip.CreateEntry($name).Open()
            try { $bytes = [byte[]]$Entries[$name]; $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
        }
    } finally { $zip.Dispose() }
}

function New-FixtureManifest {
    param([string]$Path, [string]$ZipPath, [string]$DllPath, [string]$ZipSha, [string]$DllSha)
    if (-not $ZipSha) { $ZipSha = Get-Sha256 $ZipPath }
    if (-not $DllSha) { $DllSha = Get-Sha256 $DllPath }
    $manifest = [ordered]@{
        schemaVersion = 1
        synthetic     = $true
        assets        = @(
            [ordered]@{
                id       = "ort-runtime/$version/win-x64"
                kind     = 'ort-runtime'
                identity = [ordered]@{ version = $version; platform = 'win'; arch = 'x64' }
                files    = @(
                    [ordered]@{ name = $zipName; url = $zipUrl; size = (Get-Item -LiteralPath $ZipPath).Length; sha256 = $ZipSha },
                    [ordered]@{ name = 'onnxruntime.dll'; fromArchive = $zipName; archivePath = $archivePath
                        size = (Get-Item -LiteralPath $DllPath).Length; sha256 = $DllSha }
                )
                approval = [ordered]@{
                    status     = 'approved'
                    approvedBy = 'SYNTHETIC-FIXTURE'
                    approvedAt = '2026-10-22'
                    method     = 'SYNTHETIC test data only; no real review was performed'
                    evidence   = @('SYNTHETIC: hashes describe generated test files only')
                }
            }
        )
    }
    [IO.File]::WriteAllText($Path, ($manifest | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
}

# ---- 合成 fixture ----
$goodDllBytes = [Text.Encoding]::ASCII.GetBytes('SYNTHETIC onnxruntime.dll v1 - not a real library')
$oldDllBytes = [Text.Encoding]::ASCII.GetBytes('SYNTHETIC previously installed DLL - must survive failures')
$goodDll = Join-Path $fixtures 'good.dll'
[IO.File]::WriteAllBytes($goodDll, $goodDllBytes)

$goodZip = Join-Path $fixtures 'good.zip'
New-FixtureZip $goodZip @{ $archivePath = $goodDllBytes; "$package/README.md" = [Text.Encoding]::ASCII.GetBytes('SYNTHETIC') }
$otherZip = Join-Path $fixtures 'other.zip'      # 内容不同的 zip（错误 archive hash）
New-FixtureZip $otherZip @{ $archivePath = [Text.Encoding]::ASCII.GetBytes('SYNTHETIC tampered DLL bytes') }
$truncatedZip = Join-Path $fixtures 'truncated.zip'
$goodZipBytes = [IO.File]::ReadAllBytes($goodZip)
[IO.File]::WriteAllBytes($truncatedZip, $goodZipBytes[0..($goodZipBytes.Length - 11)])
$missingZip = Join-Path $fixtures 'missing-entry.zip'  # 只有大小写不同的路径，指定内部路径缺失
New-FixtureZip $missingZip @{ "$package/Lib/onnxruntime.dll" = $goodDllBytes }
$wrongDllZip = Join-Path $fixtures 'wrong-dll.zip'     # zip 本身获批，但内部 DLL 与批准摘要不符
$wrongDllBytes = [byte[]]$goodDllBytes.Clone(); $wrongDllBytes[$wrongDllBytes.Length - 1] = [byte]0x21  # 同大小、摘要不同
New-FixtureZip $wrongDllZip @{ $archivePath = $wrongDllBytes }
$oversizedZip = Join-Path $fixtures 'oversized-dll.zip'  # 内部 DLL 超出批准大小
New-FixtureZip $oversizedZip @{ $archivePath = [byte[]]($goodDllBytes + [byte[]](0x20, 0x20)) }

$goodManifest = Join-Path $fixtures 'good-manifest.json'
New-FixtureManifest $goodManifest $goodZip $goodDll
$missingManifest = Join-Path $fixtures 'missing-entry-manifest.json'
New-FixtureManifest $missingManifest $missingZip $goodDll
$wrongDllManifest = Join-Path $fixtures 'wrong-dll-manifest.json'
New-FixtureManifest $wrongDllManifest $wrongDllZip $goodDll
$oversizedManifest = Join-Path $fixtures 'oversized-dll-manifest.json'
New-FixtureManifest $oversizedManifest $oversizedZip $goodDll
$wrongZipHashManifest = Join-Path $fixtures 'wrong-zip-hash-manifest.json'
New-FixtureManifest $wrongZipHashManifest $goodZip $goodDll -ZipSha ('0' * 64)

# ---- 网络替身 ----
$TestState = @{ DownloadSource = $null; DownloadCalls = 0 }
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    $TestState.DownloadCalls++
    if ($Uri -cne 'https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip') { throw "TEST: unexpected download URL $Uri" }
    if ([IO.Path]::GetFileName($OutFile) -cne 'onnxruntime-win-x64-1.20.1.zip') { throw 'TEST: unexpected archive name' }
    if (-not $TestState.DownloadSource) { throw 'TEST: simulated download failure' }
    [IO.File]::Copy($TestState.DownloadSource, $OutFile)
}

$count = 0
function New-Case {
    param([string]$Name, [byte[]]$ExistingDll)
    $dir = Join-Path $root ('case-' + $Name)
    if ($null -ne $ExistingDll) {
        New-Item -ItemType Directory $dir | Out-Null
        [IO.File]::WriteAllBytes((Join-Path $dir 'onnxruntime.dll'), $ExistingDll)
    }
    $TestState.DownloadCalls = 0
    return $dir
}

function Invoke-Prepare {
    param([string]$Dir, [string]$Manifest, [string]$Version = '1.20.1', [string]$Arch = 'x64', [switch]$NoMockLoader,
        [ValidateSet('', 'false', 'throw', 'recovery-failure')][string]$PostCheck = '')
    foreach ($path in @($Dir, $Manifest)) {
        if (-not [IO.Path]::GetFullPath($path).StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'TEST: path outside sandbox'
        }
    }
    if (-not $NoMockLoader) {
        # 本地作用域屏蔽重复导入；真实模块已加载，所有其他函数继续使用生产实现。
        function Import-Module {
            param([string]$Name, [switch]$Force)
            if ([IO.Path]::GetFullPath($Name) -ine $modulePath) { throw 'TEST: unexpected module' }
        }
        function Import-AssetManifest {
            param([string]$Path)
            if ($Path -cne $Manifest) { throw 'TEST: unexpected manifest path' }
            AssetManifest\Import-AssetManifest -Path $Path -AllowSynthetic
        }
    }
    $injection = @{ Lock = $null; Hits = 0 }
    function Test-AssetFile {
        param([string]$Path, $FileEntry)
        $valid = AssetManifest\Test-AssetFile -Path $Path -FileEntry $FileEntry
        # 初始旧 DLL 不匹配；只在真实替换成功后的最终目标检查注入。
        if ($PostCheck -and $Path -ieq (Join-Path $Dir 'onnxruntime.dll') -and $valid) {
            $injection.Hits++
            if ($PostCheck -eq 'recovery-failure') {
                $injection.Lock = [IO.File]::Open($Path, 'Open', 'Read', 'None')
            }
            if ($PostCheck -eq 'throw') { throw 'TEST: post-install verification exception' }
            return $false
        }
        return $valid
    }
    try {
        & $scriptUnderTest -Version $Version -Arch $Arch -ManifestPath $Manifest -BinariesDir $Dir 6>$null 3>$null | Out-Null
    } finally {
        if ($injection.Lock) { $injection.Lock.Dispose() }
        if ($PostCheck -and $injection.Hits -ne 1) { throw 'TEST: post-install injection did not run exactly once' }
    }
}

function Expect-Failure {
    param([string]$Name, [scriptblock]$Action, [string]$Expected)
    $failed = $false
    try { & $Action } catch {
        $failed = $true
        if (-not $_.Exception.Message.Contains($Expected)) {
            throw "Case '$Name': expected error containing '$Expected', got: $($_.Exception.Message)"
        }
    }
    if (-not $failed) { throw "Case '$Name': expected failure ($Expected) but script succeeded" }
}

function Assert-Dll {
    param([string]$Name, [string]$Dir, [byte[]]$Expected)
    $path = Join-Path $Dir 'onnxruntime.dll'
    if ($null -eq $Expected) {
        if (Test-Path -LiteralPath $path) { throw "Case '$Name': target DLL must not exist" }
    } else {
        if (-not (Test-Path -LiteralPath $path)) { throw "Case '$Name': target DLL missing" }
        $actual = [IO.File]::ReadAllBytes($path)
        if ([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($Expected)) { throw "Case '$Name': target DLL content changed unexpectedly" }
    }
    if (Test-Path -LiteralPath $Dir) {
        $leftover = @(Get-ChildItem -LiteralPath $Dir -Force | Where-Object { $_.Name -ne 'onnxruntime.dll' })
        if ($leftover.Count -gt 0) { throw "Case '$Name': leftover staging files: $($leftover.Name -join ', ')" }
    }
}

function Assert-Downloads { param([string]$Name, [int]$Expected)
    if ($TestState.DownloadCalls -ne $Expected) { throw "Case '$Name': expected $Expected download(s), got $($TestState.DownloadCalls)" }
}

# 默认模式不 mock 计划/校验/解压/事务，只替换下载和临时 DLL 的版本资源读取。
function Invoke-DefaultPrepare {
    param([string]$Dir, [string]$Version = '1.20.1', [string]$Arch = 'x64',
        [switch]$WrongVersion, [switch]$NoVersionMock,
        [ValidateSet('', 'false', 'throw', 'recovery-failure')][string]$PostCheck = '')
    if (-not [IO.Path]::GetFullPath($Dir).StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'TEST: default output outside sandbox'
    }
    $state = @{ Hits = 0; Lock = $null }
    function Get-Item {
        param([string]$LiteralPath)
        $item = Microsoft.PowerShell.Management\Get-Item -LiteralPath $LiteralPath
        if (-not $NoVersionMock -and $item.Extension -eq '.dll') {
            if (-not $item.FullName.StartsWith($Dir + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
                throw 'TEST: version mock outside case directory'
            }
            $minor = 20
            if ($WrongVersion) { $minor = 19 }
            if ($PostCheck -and $item.FullName -ieq (Join-Path $Dir 'onnxruntime.dll')) {
                $state.Hits++
                if ($PostCheck -eq 'throw') { throw 'TEST: default final version read exception' }
                if ($PostCheck -eq 'recovery-failure') { $state.Lock = [IO.File]::Open($LiteralPath, 'Open', 'Read', 'None') }
                $minor = 19
            }
            # 与官方 1.20.1 包真实元数据一致：1.20.20241119.10（build/private 不是 1.0）。
            return [pscustomobject]@{ VersionInfo = [pscustomobject]@{
                FileMajorPart = 1; FileMinorPart = $minor; FileBuildPart = 20241119; FilePrivatePart = 10
            } }
        }
        return $item
    }
    # 若默认路径误触任何清单读取，立即失败（即使仓库碰巧有清单）。
    function Import-Module { param([string]$Name, [switch]$Force)
        if ([IO.Path]::GetFullPath($Name) -ine $modulePath) { throw 'TEST: unexpected module' }
    }
    function Import-AssetManifest { throw 'TEST: default path must not read a manifest' }
    function Get-AssetManifestDefaultPath { throw 'TEST: default path must not resolve a manifest' }
    try {
        & $scriptUnderTest -Version $Version -Arch $Arch -BinariesDir $Dir 6>$null 3>$null | Out-Null
    } finally {
        if ($state.Lock) { $state.Lock.Dispose() }
        if ($PostCheck -and $state.Hits -ne 1) { throw 'TEST: default final verification injection missed' }
    }
}

# 结构完整但不可执行的 PE fixture；只有版本资源由局部 seam 模拟。
$peBytes = New-Object byte[] 1024
function Set-FixtureNumber { param([int]$Offset, [byte[]]$Bytes) [Array]::Copy($Bytes, 0, $peBytes, $Offset, $Bytes.Length) }
Set-FixtureNumber 0 ([BitConverter]::GetBytes([uint16]0x5a4d))
Set-FixtureNumber 60 ([BitConverter]::GetBytes([uint32]128))
Set-FixtureNumber 128 ([BitConverter]::GetBytes([uint32]0x4550))
Set-FixtureNumber 132 ([BitConverter]::GetBytes([uint16]0x8664))
Set-FixtureNumber 134 ([BitConverter]::GetBytes([uint16]1))
Set-FixtureNumber 148 ([BitConverter]::GetBytes([uint16]240))
Set-FixtureNumber 150 ([BitConverter]::GetBytes([uint16]0x2022))
Set-FixtureNumber 152 ([BitConverter]::GetBytes([uint16]0x20b))
Set-FixtureNumber 408 ([BitConverter]::GetBytes([uint32]512))
Set-FixtureNumber 412 ([BitConverter]::GetBytes([uint32]512))

function New-DefaultZip {
    param([string]$Path, [string]$Entry = $archivePath, [switch]$Duplicate)
    # 本地无压缩 filler 保持生产已知 size 校验真实执行；没有网络，也不伪造 hash。
    $padding = [long]0
    for ($pass = 0; $pass -lt 5; $pass++) {
        if ($pass -gt 0) {
            $delta = 65585496 - (Get-Item -LiteralPath $Path).Length
            if ($delta -eq 0) { break }
            # .NET Framework 的 NoCompression 仍有 deflate block 开销，按实际长度收敛。
            $padding += $delta
            Remove-Item -LiteralPath $Path
        }
        $fillerLength = $padding
        $zip = [IO.Compression.ZipFile]::Open($Path, [IO.Compression.ZipArchiveMode]::Create)
        try {
            $copies = 1
            if ($Duplicate) { $copies = 2 }
            for ($i = 0; $i -lt $copies; $i++) {
                $stream = $zip.CreateEntry($Entry, [IO.Compression.CompressionLevel]::NoCompression).Open()
                try { $stream.Write($peBytes, 0, $peBytes.Length) } finally { $stream.Dispose() }
            }
            $stream = $zip.CreateEntry('SYNTHETIC-padding', [IO.Compression.CompressionLevel]::NoCompression).Open()
            try {
                $buffer = New-Object byte[] 65536
                while ($fillerLength -gt 0) {
                    $n = [int][Math]::Min($buffer.Length, $fillerLength)
                    $stream.Write($buffer, 0, $n)
                    $fillerLength -= $n
                }
            } finally { $stream.Dispose() }
        } finally { $zip.Dispose() }
    }
    if ((Get-Item -LiteralPath $Path).Length -ne 65585496) { throw "TEST: default fixture size incorrect: $((Get-Item -LiteralPath $Path).Length)" }
}

try {
    # 1. 正确：无目标时下载、校验 zip 与内部 DLL 后安装。
    $dir = New-Case 'fresh-install'
    $TestState.DownloadSource = $goodZip
    Invoke-Prepare $dir $goodManifest
    Assert-Dll 'fresh-install' $dir $goodDllBytes; Assert-Downloads 'fresh-install' 1; $count++

    # 2. 已有正确 DLL：离线复用，不触发下载。
    $dir = New-Case 'verified-reuse' $goodDllBytes
    $TestState.DownloadSource = $null
    Invoke-Prepare $dir $goodManifest
    Assert-Dll 'verified-reuse' $dir $goodDllBytes; Assert-Downloads 'verified-reuse' 0; $count++

    # 3. 已有错误 DLL：不能凭存在复用，验证下载成功后替换。
    $dir = New-Case 'replace-wrong-existing' $oldDllBytes
    $TestState.DownloadSource = $goodZip
    Invoke-Prepare $dir $goodManifest
    Assert-Dll 'replace-wrong-existing' $dir $goodDllBytes; Assert-Downloads 'replace-wrong-existing' 1; $count++

    # 4. 已有错误 DLL 且下载失败：非零失败，旧 DLL 保留（未被删除或以假乱真）。
    $dir = New-Case 'download-failure' $oldDllBytes
    $TestState.DownloadSource = $null
    Expect-Failure 'download-failure' { Invoke-Prepare $dir $goodManifest } 'simulated download failure'
    Assert-Dll 'download-failure' $dir $oldDllBytes; Assert-Downloads 'download-failure' 1; $count++

    # 5. 无目标且下载失败：不留下目标或临时文件。
    $dir = New-Case 'download-failure-empty'
    Expect-Failure 'download-failure-empty' { Invoke-Prepare $dir $goodManifest } 'simulated download failure'
    Assert-Dll 'download-failure-empty' $dir $null; $count++

    # 6. 错误 zip 内容（同名不同包）：sha256/size 不符拒绝，旧 DLL 保留。
    $dir = New-Case 'wrong-zip-content' $oldDllBytes
    $TestState.DownloadSource = $otherZip
    Expect-Failure 'wrong-zip-content' { Invoke-Prepare $dir $goodManifest } 'mismatch'
    Assert-Dll 'wrong-zip-content' $dir $oldDllBytes; $count++

    # 7. 错误 zip hash（大小相同、摘要不符）：明确报 sha256 mismatch。
    $dir = New-Case 'wrong-zip-hash' $oldDllBytes
    $TestState.DownloadSource = $goodZip
    Expect-Failure 'wrong-zip-hash' { Invoke-Prepare $dir $wrongZipHashManifest } 'sha256 mismatch'
    Assert-Dll 'wrong-zip-hash' $dir $oldDllBytes; $count++

    # 8. 截断 zip：大小不符拒绝。
    $dir = New-Case 'truncated-zip' $oldDllBytes
    $TestState.DownloadSource = $truncatedZip
    Expect-Failure 'truncated-zip' { Invoke-Prepare $dir $goodManifest } 'size mismatch'
    Assert-Dll 'truncated-zip' $dir $oldDllBytes; $count++

    # 9. zip 获批但缺少指定内部路径（仅大小写不同也不接受）。
    $dir = New-Case 'missing-internal-path' $oldDllBytes
    $TestState.DownloadSource = $missingZip
    Expect-Failure 'missing-internal-path' { Invoke-Prepare $dir $missingManifest } "does not contain $archivePath"
    Assert-Dll 'missing-internal-path' $dir $oldDllBytes; $count++

    # 10. zip 获批但内部 DLL 与批准摘要不符。
    $dir = New-Case 'wrong-internal-dll' $oldDllBytes
    $TestState.DownloadSource = $wrongDllZip
    Expect-Failure 'wrong-internal-dll' { Invoke-Prepare $dir $wrongDllManifest } 'sha256 mismatch'
    Assert-Dll 'wrong-internal-dll' $dir $oldDllBytes; $count++

    # 10b. 内部 DLL 超出批准大小：解压时即中止。
    $dir = New-Case 'oversized-internal-dll' $oldDllBytes
    $TestState.DownloadSource = $oversizedZip
    Expect-Failure 'oversized-internal-dll' { Invoke-Prepare $dir $oversizedManifest } 'larger than the approved size'
    Assert-Dll 'oversized-internal-dll' $dir $oldDllBytes; $count++

    # 11/12. 未批准的 Version / Arch override：下载前拒绝，不创建目录。
    foreach ($override in @(@{ Name = 'unapproved-version'; Version = '1.20.2'; Arch = 'x64' },
                            @{ Name = 'unapproved-arch'; Version = '1.20.1'; Arch = 'arm64' },
                            @{ Name = 'case-variant-arch'; Version = '1.20.1'; Arch = 'X64' })) {
        $dir = New-Case $override.Name
        $TestState.DownloadSource = $goodZip
        Expect-Failure $override.Name { Invoke-Prepare $dir $goodManifest -Version $override.Version -Arch $override.Arch } 'no approved asset'
        Assert-Downloads $override.Name 0
        if (Test-Path -LiteralPath $dir) { throw "Case '$($override.Name)': binaries dir must not be created" }
        $count++
    }

    # 14. 清单缺失：拒绝，旧 DLL 保留、无下载。
    $dir = New-Case 'manifest-missing' $oldDllBytes
    Expect-Failure 'manifest-missing' { Invoke-Prepare $dir (Join-Path $fixtures 'does-not-exist.json') } 'manifest file not found'
    Assert-Dll 'manifest-missing' $dir $oldDllBytes; Assert-Downloads 'manifest-missing' 0; $count++

    # 15. 不使用 mock loader 的生产路径必须拒绝 synthetic 清单。
    $dir = New-Case 'synthetic-not-allowed' $goodDllBytes
    Expect-Failure 'synthetic-not-allowed' { Invoke-Prepare $dir $goodManifest -NoMockLoader } 'synthetic test manifest'
    Assert-Downloads 'synthetic-not-allowed' 0; $count++

    # 16/17. 以 -File / -Command 子进程运行，确认失败时进程退出码非零（同一 PowerShell 宿主）。
    $hostExe = (Get-Process -Id $PID).Path
    # Windows PowerShell 5.1 在 EAP=Stop 下会把被重定向的原生 stderr 变成终止错误，子进程调用局部改为 Continue。
    function Invoke-Child {
        $ErrorActionPreference = 'Continue'
        & $hostExe -NoProfile -ExecutionPolicy Bypass @args *> $null
        return $LASTEXITCODE
    }
    $dir = New-Case 'exit-code-manifest-missing' $oldDllBytes
    $code = Invoke-Child -File $scriptUnderTest -ManifestPath (Join-Path $fixtures 'does-not-exist.json') -BinariesDir $dir
    if ($code -eq 0) { throw "Case 'exit-code-manifest-missing': expected non-zero exit code" }
    Assert-Dll 'exit-code-manifest-missing' $dir $oldDllBytes; $count++

    # 默认输出路径不能因缺清单或 synthetic 清单产生任何文件/目录变化。
    function Get-TreeSnapshot {
        param([string]$Path)
        if (-not (Test-Path -LiteralPath $Path)) { return 'ABSENT' }
        return 'PRESENT:' + ((Get-ChildItem -LiteralPath $Path -Recurse -Force | Sort-Object FullName | ForEach-Object {
            $value = 'DIRECTORY'
            if (-not $_.PSIsContainer) { $value = Get-Sha256 $_.FullName }
            $_.FullName.Substring($Path.Length) + '=' + $value
        }) -join "`n")
    }
    $productionDir = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../src-tauri/binaries'))
    $before = Get-TreeSnapshot $productionDir
    $code = Invoke-Child -File $scriptUnderTest -ManifestPath $goodManifest
    if ($code -eq 0) { throw 'Production CLI accepted synthetic manifest with default output' }
    $count++
    $code = Invoke-Child -File $scriptUnderTest -ManifestPath (Join-Path $fixtures 'does-not-exist.json')
    if ($code -eq 0) { throw 'Explicit missing manifest must fail with default output' }
    $count++
    # 无参数生产 CLI 现在会下载官方资产，绝不能在离线测试中启动它。
    # 默认无清单的生产下载 seam 在下方临时目录内覆盖。
    if ((Get-TreeSnapshot $productionDir) -cne $before) { throw 'Production binaries changed after rejected CLI calls' }

    # 生产 CLI 无论是否指定输出目录均拒绝 synthetic；旧开关已移除。
    foreach ($extra in @(@{ Args = @() }, @{ Args = @('-AllowSyntheticManifest') })) {
        $dir = New-Case ('cli-synthetic-' + $count) $oldDllBytes
        $code = Invoke-Child -File $scriptUnderTest -ManifestPath $goodManifest -BinariesDir $dir @($extra.Args)
        if ($code -eq 0) { throw 'CLI accepted synthetic manifest or removed switch' }
        Assert-Dll 'cli-synthetic' $dir $oldDllBytes; $count++
    }
    if ((Get-Command $scriptUnderTest).Parameters.ContainsKey('AllowSyntheticManifest')) { throw 'Production synthetic switch remains' }

    foreach ($mode in @('false', 'throw')) {
        $dir = New-Case ('post-check-' + $mode) $oldDllBytes
        $TestState.DownloadSource = $goodZip
        $expected = 'failed post-install verification'
        if ($mode -eq 'throw') { $expected = 'post-install verification exception' }
        Expect-Failure $mode { Invoke-Prepare $dir $goodManifest -PostCheck $mode } $expected
        Assert-Dll $mode $dir $oldDllBytes
        Assert-Downloads $mode 1; $count++
        $dir = New-Case ('post-check-empty-' + $mode)
        Expect-Failure ($mode + '-empty') { Invoke-Prepare $dir $goodManifest -PostCheck $mode } $expected
        Assert-Dll ($mode + '-empty') $dir $null; $count++
    }
    $dir = New-Case 'recovery-failure' $oldDllBytes
    $TestState.DownloadSource = $goodZip
    $message = ''
    try { Invoke-Prepare $dir $goodManifest -PostCheck 'recovery-failure' } catch { $message = $_.Exception.Message }
    $backups = @(Get-ChildItem -LiteralPath $dir -Recurse -Filter 'onnxruntime.dll.previous')
    if ($backups.Count -ne 1) { throw 'Recovery failure deleted the unique old DLL backup' }
    if (-not $message.Contains('Recovery backup retained at ' + $backups[0].FullName)) { throw "Backup location missing from error: $message" }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($backups[0].FullName)) -cne [Convert]::ToBase64String($oldDllBytes)) { throw 'Recovery backup bytes changed' }
    $count++
    Write-Output "PASS: $count explicit-manifest cases"

    $defaultZip = Join-Path $fixtures 'default.zip'
    New-DefaultZip $defaultZip
    $dir = New-Case 'default-fresh'
    $TestState.DownloadSource = $defaultZip
    Invoke-DefaultPrepare $dir
    Assert-Dll 'default-fresh' $dir $peBytes; Assert-Downloads 'default-fresh' 1; $count++

    $dir = New-Case 'default-reuse' $peBytes
    $TestState.DownloadSource = $null
    Invoke-DefaultPrepare $dir
    Assert-Dll 'default-reuse' $dir $peBytes; Assert-Downloads 'default-reuse' 0; $count++

    $dir = New-Case 'default-replace' $oldDllBytes
    $TestState.DownloadSource = $defaultZip
    Invoke-DefaultPrepare $dir
    Assert-Dll 'default-replace' $dir $peBytes; Assert-Downloads 'default-replace' 1; $count++

    foreach ($override in @(@{ Version = '1.20.2'; Arch = 'x64' }, @{ Version = '1.20.1'; Arch = 'arm64' }, @{ Version = '1.20.1'; Arch = 'X64' })) {
        $dir = New-Case ('default-override-' + $count)
        Expect-Failure 'default-override' { Invoke-DefaultPrepare $dir -Version $override.Version -Arch $override.Arch } 'require explicit -ManifestPath'
        Assert-Downloads 'default-override' 0
        if (Test-Path -LiteralPath $dir) { throw 'Default override created a directory' }
        $count++
    }

    $dir = New-Case 'default-download-failure' $oldDllBytes
    $TestState.DownloadSource = $null
    Expect-Failure 'default-download-failure' { Invoke-DefaultPrepare $dir } 'simulated download failure'
    Assert-Dll 'default-download-failure' $dir $oldDllBytes; Assert-Downloads 'default-download-failure' 1; $count++

    $dir = New-Case 'default-existing-version-mismatch' $peBytes
    Expect-Failure 'default-existing-version-mismatch' { Invoke-DefaultPrepare $dir -WrongVersion } 'simulated download failure'
    Assert-Dll 'default-existing-version-mismatch' $dir $peBytes; Assert-Downloads 'default-existing-version-mismatch' 1; $count++

    $dir = New-Case 'default-version-mismatch' $oldDllBytes
    $TestState.DownloadSource = $defaultZip
    Expect-Failure 'default-version-mismatch' { Invoke-DefaultPrepare $dir -WrongVersion } 'PE/version mismatch'
    Assert-Dll 'default-version-mismatch' $dir $oldDllBytes; $count++

    # 有 PE 头但没有版本资源不应复用，也不应安装（真实 FileVersionInfo，无 mock）。
    $dir = New-Case 'default-no-version-resource' $peBytes
    Expect-Failure 'default-no-version-resource' { Invoke-DefaultPrepare $dir -NoVersionMock } 'PE/version mismatch'
    Assert-Dll 'default-no-version-resource' $dir $peBytes; Assert-Downloads 'default-no-version-resource' 1; $count++

    $dir = New-Case 'default-small-zip' $oldDllBytes
    $TestState.DownloadSource = $goodZip
    Expect-Failure 'default-small-zip' { Invoke-DefaultPrepare $dir } 'archive size mismatch'
    Assert-Dll 'default-small-zip' $dir $oldDllBytes; $count++

    foreach ($kind in @('missing', 'duplicate', 'bad-pe')) {
        Remove-Item -LiteralPath $defaultZip
        if ($kind -eq 'missing') { New-DefaultZip $defaultZip -Entry '../onnxruntime.dll'; $expected = 'does not contain' }
        elseif ($kind -eq 'duplicate') { New-DefaultZip $defaultZip -Duplicate; $expected = 'duplicate entries' }
        else {
            $peBytes[0] = 0
            try { New-DefaultZip $defaultZip } finally { $peBytes[0] = 0x4d }
            $expected = 'PE/version mismatch'
        }
        $dir = New-Case ('default-' + $kind) $oldDllBytes
        $TestState.DownloadSource = $defaultZip
        Expect-Failure $kind { Invoke-DefaultPrepare $dir } $expected
        Assert-Dll $kind $dir $oldDllBytes; $count++
    }
    Remove-Item -LiteralPath $defaultZip
    New-DefaultZip $defaultZip
    $TestState.DownloadSource = $defaultZip
    foreach ($mode in @('false', 'throw')) {
        foreach ($existing in @($true, $false)) {
            $dir = New-Case ('default-post-' + $count) $(if ($existing) { $oldDllBytes } else { $null })
            $expected = 'failed post-install verification'
            if ($mode -eq 'throw') { $expected = 'default final version read exception' }
            Expect-Failure $mode { Invoke-DefaultPrepare $dir -PostCheck $mode } $expected
            Assert-Dll $mode $dir $(if ($existing) { $oldDllBytes } else { $null })
            Assert-Downloads $mode 1; $count++
        }
    }
    $dir = New-Case 'default-recovery-failure' $oldDllBytes
    $message = ''
    try { Invoke-DefaultPrepare $dir -PostCheck 'recovery-failure' } catch { $message = $_.Exception.Message }
    $backups = @(Get-ChildItem -LiteralPath $dir -Recurse -Filter 'onnxruntime.dll.previous')
    if ($backups.Count -ne 1) { throw 'Default recovery failure deleted old DLL backup' }
    if (-not $message.Contains('Recovery backup retained at ' + $backups[0].FullName)) { throw "Default backup location missing: $message" }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($backups[0].FullName)) -cne [Convert]::ToBase64String($oldDllBytes)) { throw 'Default backup bytes changed' }
    $count++
} finally {
    # 只删除本测试创建的临时根目录。
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "PASS: $count prepare_ort_runtime cases on PowerShell $($PSVersionTable.PSVersion)"
