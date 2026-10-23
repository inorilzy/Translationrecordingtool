# prepare_ocr_models.ps1 离线合成测试（工单 25），无 Pester 依赖。
# 不访问网络、不读写用户真实缓存或 src-tauri/resources：所有模型/缓存目录与清单都生成在临时目录，
# 资产内容是合成字节，清单带 "synthetic": true，hash 由测试对合成字节现场计算，不代表任何真实资产。
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/tests/Test-PrepareOcrModels.ps1
#   pwsh -NoProfile -File scripts/tests/Test-PrepareOcrModels.ps1
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$scriptPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../prepare_ocr_models.ps1'))
. $scriptPath

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ('prepare-ocr-models-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempRoot | Out-Null
$script:Passed = 0
$roles = @('det', 'rec')
$names = @('inference.onnx', 'inference.json', 'inference.yml')
$revisions = @{ det = ('a' * 40); rec = ('b' * 40) }

function Get-Sha256Hex {
    param([byte[]]$Bytes)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return (($sha.ComputeHash($Bytes) | ForEach-Object { $_.ToString('x2') }) -join '') } finally { $sha.Dispose() }
}

function Get-SyntheticBytes {
    param([string]$Role, [string]$Name, [string]$Variant = 'v1')
    return [Text.Encoding]::UTF8.GetBytes(("SYNTHETIC-OCR-FIXTURE $Role/$Name $Variant`n" * 8))
}

function Get-FileUrl {
    param([string]$Role, [string]$Name)
    return "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_${Role}_onnx/resolve/$($revisions[$Role])/$Name"
}

# 合成“服务器”：URL → 字节。测试可以按 URL 篡改、截断或令其失败。
$script:Server = @{}
$script:Downloads = New-Object System.Collections.Generic.List[string]
$script:FailUrls = @{}
function Reset-Server {
    $script:Server = @{}
    foreach ($role in $roles) { foreach ($name in $names) { $script:Server[(Get-FileUrl $role $name)] = (Get-SyntheticBytes $role $name) } }
    $script:Downloads.Clear()
    $script:FailUrls = @{}
}
$fakeDownloader = {
    param([string]$Url, [string]$OutFile)
    $script:Downloads.Add($Url)
    if ($script:FailUrls.ContainsKey($Url)) { throw "synthetic download failure for $Url" }
    if (-not $script:Server.ContainsKey($Url)) { throw "synthetic 404 for $Url" }
    [IO.File]::WriteAllBytes($OutFile, $script:Server[$Url])
}

function New-SyntheticManifest {
    param([string]$Path, [string[]]$IncludeRoles = $roles, [switch]$NotSynthetic)
    $assets = foreach ($role in $IncludeRoles) {
        $files = foreach ($name in $names) {
            $bytes = Get-SyntheticBytes $role $name
            [ordered]@{ name = $name; url = (Get-FileUrl $role $name); size = $bytes.Length; sha256 = (Get-Sha256Hex $bytes) }
        }
        [ordered]@{
            id = "ocr-model/small/$role"
            kind = 'ocr-model'
            identity = [ordered]@{ profile = 'small'; role = $role; repo = "PaddlePaddle/PP-OCRv6_small_${role}_onnx"; revision = $revisions[$role] }
            files = @($files)
            approval = [ordered]@{
                status = 'approved'; approvedBy = 'SYNTHETIC-FIXTURE'; approvedAt = '2026-10-22'
                method = 'SYNTHETIC test data only; no real review was performed'
                evidence = @('SYNTHETIC: hashes computed from generated test bytes')
            }
        }
    }
    $doc = [ordered]@{ schemaVersion = 1 }
    if (-not $NotSynthetic) { $doc['synthetic'] = $true }
    $doc['assets'] = @($assets)
    [IO.File]::WriteAllText($Path, ($doc | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
}

function New-Sandbox {
    param([string]$Name)
    $root = Join-Path $tempRoot $Name
    $sandbox = @{
        Root = $root
        ModelRoot = (Join-Path $root 'ocr-models')
        CacheRoot = (Join-Path $root 'cache')
        Manifest = (Join-Path $root 'release-assets.json')
    }
    New-Item -ItemType Directory -Path $root | Out-Null
    New-SyntheticManifest -Path $sandbox.Manifest
    Reset-Server
    return $sandbox
}

function Invoke-Prepare {
    param($Sandbox, [switch]$Force, [string]$Profile = 'small', [string]$Manifest)
    if (-not $Manifest) { $Manifest = $Sandbox.Manifest }
    # mock 仅存在于本次调用作用域，且只允许本测试临时根目录的输入/输出。
    foreach ($path in @($Manifest, $Sandbox.ModelRoot, $Sandbox.CacheRoot)) {
        if (-not [IO.Path]::GetFullPath($path).StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'TEST: path outside sandbox'
        }
    }
    function Import-AssetManifest {
        param([string]$Path)
        if ($Path -cne $Manifest) { throw 'TEST: unexpected manifest path' }
        AssetManifest\Import-AssetManifest -Path $Path -AllowSynthetic
    }
    Invoke-OcrModelPreparation -Profile $Profile -Force:$Force -ManifestPath $Manifest `
        -ModelRoot $Sandbox.ModelRoot -CacheRoot $Sandbox.CacheRoot -Downloader $fakeDownloader 6>$null
}

function Invoke-DefaultPrepare {
    param($Sandbox, [switch]$Force)
    foreach ($path in @($Sandbox.ModelRoot, $Sandbox.CacheRoot)) {
        if (-not [IO.Path]::GetFullPath($path).StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'TEST: path outside sandbox' }
    }
    # 仅替换默认计划数据；真实生产准备/缓存/事务路径保持不变，不生成 approval。
    function Get-DefaultOcrModelPlan {
        foreach ($role in $roles) {
            $files = @(foreach ($name in $names) {
                $bytes = Get-SyntheticBytes $role $name
                $url = Get-FileUrl $role $name
                @{ Name = $name; Url = $url; Entry = @{ name = $name; size = $bytes.Length; sha256 = (Get-Sha256Hex $bytes) } }
            })
            @{ Role = $role; Repo = "PaddlePaddle/PP-OCRv6_small_${role}_onnx"; Revision = $revisions[$role]; Files = $files }
        }
    }
    function Import-AssetManifest { throw 'TEST: default must not import a manifest' }
    Invoke-OcrModelPreparation -Force:$Force -ModelRoot $Sandbox.ModelRoot -CacheRoot $Sandbox.CacheRoot -Downloader $fakeDownloader 6>$null
}

function Get-TargetPath { param($Sandbox, [string]$Role, [string]$Name) Join-Path (Join-Path (Join-Path $Sandbox.ModelRoot 'small') $Role) $Name }
function Get-CachePath { param($Sandbox, [string]$Role, [string]$Name) Join-Path (Join-Path (Join-Path $Sandbox.CacheRoot "PaddlePaddle_PP-OCRv6_small_${Role}_onnx") $revisions[$Role]) $Name }

function Assert-True { param([bool]$Condition, [string]$Message) if (-not $Condition) { throw "Assertion failed: $Message" } }

function Assert-TargetValid {
    param($Sandbox)
    foreach ($role in $roles) { foreach ($name in $names) {
        $path = Get-TargetPath $Sandbox $role $name
        Assert-True (Test-Path -LiteralPath $path) "target $role/$name exists"
        $actual = Get-Sha256Hex ([IO.File]::ReadAllBytes($path))
        Assert-True ($actual -ceq (Get-Sha256Hex (Get-SyntheticBytes $role $name))) "target $role/$name content matches manifest"
    } }
    Assert-True (@(Get-ChildItem -LiteralPath $Sandbox.ModelRoot -Force -Filter '.ocr-staging-*').Count -eq 0) 'staging directory cleaned up'
}

function Get-TreeSnapshot {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return 'ABSENT' }
    return 'PRESENT:' + ((Get-ChildItem -LiteralPath $Path -Recurse -Force | Sort-Object FullName | ForEach-Object {
        $value = 'DIRECTORY'
        if (-not $_.PSIsContainer) { $value = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
        $_.FullName.Substring($Path.Length) + '=' + $value
    }) -join "`n")
}

function Expect-Failure {
    param([scriptblock]$Action, [string]$Expected)
    $failed = $false
    try { & $Action | Out-Null } catch {
        $failed = $true
        if (-not $_.Exception.Message.Contains($Expected)) { throw "Expected error containing '$Expected', got: $($_.Exception.Message)" }
    }
    if (-not $failed) { throw "Expected failure containing '$Expected', but the call succeeded" }
}

function Invoke-ChildScript {
    # Windows PowerShell 5.1 在 Stop 下会把子进程 stderr（2>&1）当作终止错误，这里局部改为 Continue；
    # 逐条转字符串而不用 Out-String，避免 5.1 按控制台宽度折行 ErrorRecord 文本。
    param([string]$HostPath, [string[]]$Arguments)
    $ErrorActionPreference = 'Continue'
    $lines = & $HostPath -NoProfile -ExecutionPolicy Bypass -File $scriptPath @Arguments 2>&1 | ForEach-Object { [string]$_ }
    return ($lines -join "`n")
}

function Test-Case {
    param([string]$Name, [scriptblock]$Body)
    try { & $Body } catch { throw "Case '$Name' failed: $($_.Exception.Message)" }
    $script:Passed++
    Write-Host "PASS $Name"
}

try {
    Test-Case 'built-in official single-channel pins match recorded revisions sizes hashes and URLs' {
        $expected = @(
            @('det', '28fe5895c24fd108c19eb3e8479f4ab385fbfc62', @(9880512, 229307, 885), @('d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e', '89240f689a4a77aad75ef55a8df0a15c8e1d4980a327d17e58f24bbadde5aeab', '193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516')),
            @('rec', 'b8f84f0b80c529de40b4fbb3544b84fa7233a513', @(21159378, 208004, 150579), @('5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634', 'f0bf53c853937a917affdd74467472167727f8ab0f0f7bded01c4a16c27e46e6', 'ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1'))
        )
        $plans = @(Get-DefaultOcrModelPlan)
        Assert-True ($plans.Count -eq 2) 'exactly two roles'
        for ($r = 0; $r -lt 2; $r++) {
            $plan = $plans[$r]; $pin = $expected[$r]
            Assert-True ($plan.Role -ceq $pin[0] -and $plan.Revision -ceq $pin[1]) 'role and revision'
            Assert-True ($plan.Repo -ceq "PaddlePaddle/PP-OCRv6_small_$($pin[0])_onnx") 'official repo'
            Assert-True ($plan.Files.Count -eq 3 -and -not $plan.ContainsKey('approval')) 'three files, no fabricated approval'
            for ($i = 0; $i -lt 3; $i++) {
                $file = $plan.Files[$i]
                Assert-True ($file.Name -ceq $names[$i] -and $file.Entry.size -eq $pin[2][$i] -and $file.Entry.sha256 -ceq $pin[3][$i]) 'name size sha256 pin'
                Assert-True ($file.Url -ceq "https://huggingface.co/$($plan.Repo)/resolve/$($pin[1])/$($names[$i])" -and $file.Entry.url -ceq $file.Url) 'fixed official URL'
            }
        }
    }

    Test-Case 'default without manifest installs and reuses verified targets through production seam' {
        $s = New-Sandbox 'default-success'
        Remove-Item -LiteralPath $s.Manifest
        Invoke-DefaultPrepare $s
        Assert-TargetValid $s
        Assert-True ($script:Downloads.Count -eq 6) 'six fake downloads without manifest'
        $script:Downloads.Clear()
        Invoke-DefaultPrepare $s
        Assert-True ($script:Downloads.Count -eq 0) 'verified default target skipped'
    }

    Test-Case 'default corrupt or failed download preserves previous targets and cache' {
        $s = New-Sandbox 'default-failure'
        Remove-Item -LiteralPath $s.Manifest
        Invoke-DefaultPrepare $s
        $before = Get-TreeSnapshot $s.ModelRoot
        $cacheBefore = Get-TreeSnapshot $s.CacheRoot
        $url = Get-FileUrl 'rec' 'inference.yml'
        $script:Server[$url] = Get-SyntheticBytes 'rec' 'inference.yml' 'v2'
        Expect-Failure { Invoke-DefaultPrepare $s -Force } 'sha256 mismatch'
        Assert-True ((Get-TreeSnapshot $s.ModelRoot) -ceq $before) 'corrupt download retains old targets'
        Assert-True ((Get-TreeSnapshot $s.CacheRoot) -ceq $cacheBefore) 'corrupt download retains cache'
        Reset-Server
        $script:FailUrls[$url] = $true
        Expect-Failure { Invoke-DefaultPrepare $s -Force } 'synthetic download failure'
        Assert-True ((Get-TreeSnapshot $s.ModelRoot) -ceq $before) 'failed download retains old targets'
        Assert-True ((Get-TreeSnapshot $s.CacheRoot) -ceq $cacheBefore) 'failed download retains cache'
        Assert-TargetValid $s
    }

    Test-Case 'correct manifest installs all det/rec files from pinned revision URLs' {
        $s = New-Sandbox 'correct'
        Invoke-Prepare $s
        Assert-TargetValid $s
        Assert-True ($script:Downloads.Count -eq 6) "six downloads (got $($script:Downloads.Count))"
        foreach ($url in $script:Downloads) { Assert-True ($url -cmatch '/resolve/[0-9a-f]{40}/inference\.(onnx|json|yml)$') "pinned url $url" }
        foreach ($role in $roles) { foreach ($name in $names) { Assert-True (Test-Path -LiteralPath (Get-CachePath $s $role $name)) "cache $role/$name" } }
    }

    Test-Case 'verified target is re-hashed and skipped without downloading' {
        $s = New-Sandbox 'rerun'
        Invoke-Prepare $s
        $script:Downloads.Clear()
        Invoke-Prepare $s
        Assert-True ($script:Downloads.Count -eq 0) 'no downloads on verified rerun'
        Assert-TargetValid $s
    }

    Test-Case 'same-size wrong-hash target file is not trusted and is reinstalled from verified cache' {
        $s = New-Sandbox 'target-wrong-hash'
        Invoke-Prepare $s
        $path = Get-TargetPath $s 'rec' 'inference.yml'
        $bytes = [IO.File]::ReadAllBytes($path); $bytes[0] = $bytes[0] -bxor 0xFF
        [IO.File]::WriteAllBytes($path, $bytes)
        $script:Downloads.Clear()
        Invoke-Prepare $s
        Assert-True ($script:Downloads.Count -eq 0) 'repaired from cache without network'
        Assert-TargetValid $s
    }

    Test-Case 'missing target file (yml/json) triggers reinstall, not skip on onnx presence' {
        $s = New-Sandbox 'target-missing'
        Invoke-Prepare $s
        Remove-Item -LiteralPath (Get-TargetPath $s 'det' 'inference.yml')
        Remove-Item -LiteralPath (Get-TargetPath $s 'det' 'inference.json')
        Invoke-Prepare $s
        Assert-TargetValid $s
    }

    Test-Case 'truncated cache file is re-downloaded; valid cache entries are reused' {
        $s = New-Sandbox 'cache-truncated'
        Invoke-Prepare $s
        $cache = Get-CachePath $s 'det' 'inference.onnx'
        $bytes = [IO.File]::ReadAllBytes($cache)
        [IO.File]::WriteAllBytes($cache, $bytes[0..9])
        Remove-Item -LiteralPath (Join-Path $s.ModelRoot 'small') -Recurse
        $script:Downloads.Clear()
        Invoke-Prepare $s
        Assert-True ($script:Downloads.Count -eq 1 -and $script:Downloads[0] -ceq (Get-FileUrl 'det' 'inference.onnx')) "only truncated file downloaded (got $($script:Downloads -join ', '))"
        Assert-True ((Get-Item -LiteralPath $cache).Length -eq $bytes.Length) 'cache repaired'
        Assert-TargetValid $s
    }

    Test-Case 'wrong hash from download fails closed and does not poison cache' {
        $s = New-Sandbox 'download-wrong-hash'
        $url = Get-FileUrl 'rec' 'inference.onnx'
        $script:Server[$url] = Get-SyntheticBytes 'rec' 'inference.onnx' 'v2'   # 同长度、不同内容
        Expect-Failure { Invoke-Prepare $s } 'sha256 mismatch'
        Assert-True (-not (Test-Path -LiteralPath (Get-CachePath $s 'rec' 'inference.onnx'))) 'tampered file not cached'
        Assert-True (@(Get-ChildItem -LiteralPath (Split-Path (Get-CachePath $s 'rec' 'inference.onnx')) -Filter '*.download-*' -Force).Count -eq 0) 'temp download removed'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $s.ModelRoot 'small\det'))) 'no partial install of det'
    }

    Test-Case 'truncated download fails closed on size check' {
        $s = New-Sandbox 'download-truncated'
        $url = Get-FileUrl 'det' 'inference.json'
        $full = $script:Server[$url]; $script:Server[$url] = $full[0..($full.Length - 5)]
        Expect-Failure { Invoke-Prepare $s } 'size mismatch'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $s.ModelRoot 'small'))) 'nothing installed'
    }

    Test-Case 'identity mismatch: rec content served for det url is rejected' {
        $s = New-Sandbox 'identity-swap'
        $script:Server[(Get-FileUrl 'det' 'inference.onnx')] = Get-SyntheticBytes 'rec' 'inference.onnx'
        Expect-Failure { Invoke-Prepare $s } 'sha256 mismatch'
    }

    Test-Case 'identity mismatch: unsupported or legacy profiles are rejected before any download' {
        $s = New-Sandbox 'identity-profile'
        foreach ($p in @('tiny', 'medium', 'lite', 'standard', 'accurate', 'Small', '')) {
            Expect-Failure { Invoke-Prepare $s -Profile $p } 'unsupported OCR model profile'
        }
        Assert-True ($script:Downloads.Count -eq 0) 'no downloads'
        Assert-True (-not (Test-Path -LiteralPath $s.ModelRoot)) 'model root untouched'
    }

    Test-Case 'manifest missing rec entry is rejected before any download' {
        $s = New-Sandbox 'manifest-missing-entry'
        New-SyntheticManifest -Path $s.Manifest -IncludeRoles @('det')
        Expect-Failure { Invoke-Prepare $s } 'no approved asset'
        Assert-True ($script:Downloads.Count -eq 0) 'no downloads'
        Assert-True (-not (Test-Path -LiteralPath $s.ModelRoot)) 'model root untouched'
    }

    Test-Case 'download failure with -Force keeps previous valid target and cache' {
        $s = New-Sandbox 'download-failure'
        Invoke-Prepare $s
        $before = Get-TreeSnapshot $s.ModelRoot
        $cacheBefore = Get-TreeSnapshot $s.CacheRoot
        $script:FailUrls[(Get-FileUrl 'rec' 'inference.yml')] = $true
        Expect-Failure { Invoke-Prepare $s -Force } 'synthetic download failure'
        Assert-True ((Get-TreeSnapshot $s.ModelRoot) -ceq $before) 'target tree unchanged'
        Assert-True ((Get-TreeSnapshot $s.CacheRoot) -ceq $cacheBefore) 'cache unchanged'
        Assert-TargetValid $s
    }

    Test-Case 'wrong-hash download under -Force keeps previous target (both roles)' {
        $s = New-Sandbox 'force-wrong-hash'
        Invoke-Prepare $s
        $before = Get-TreeSnapshot $s.ModelRoot
        $script:Server[(Get-FileUrl 'rec' 'inference.json')] = Get-SyntheticBytes 'rec' 'inference.json' 'v2'
        Expect-Failure { Invoke-Prepare $s -Force } 'sha256 mismatch'
        Assert-True ((Get-TreeSnapshot $s.ModelRoot) -ceq $before) 'det and rec targets unchanged'
    }

    Test-Case '-Force re-downloads every file even with valid cache and target, and verifies' {
        $s = New-Sandbox 'force'
        Invoke-Prepare $s
        $script:Downloads.Clear()
        Invoke-Prepare $s -Force
        Assert-True ($script:Downloads.Count -eq 6) "six downloads under -Force (got $($script:Downloads.Count))"
        Assert-TargetValid $s
    }

    Test-Case 'install failure rolls back already-replaced role and keeps old target' {
        $s = New-Sandbox 'install-rollback'
        Invoke-Prepare $s
        # 两个角色的 yml 都改坏使其“待安装”；独占打开 rec 的 yml（大小校验先失败，不会被读取），
        # 使 rec 目录重命名失败，此时 det 已被替换，必须回滚。
        foreach ($role in $roles) { [IO.File]::WriteAllText((Get-TargetPath $s $role 'inference.yml'), "OLD-$role") }
        $before = Get-TreeSnapshot $s.ModelRoot
        $lock = [IO.File]::Open((Get-TargetPath $s 'rec' 'inference.yml'), 'Open', 'Read', 'None')
        try {
            $failed = $false
            $message = ''
            try { Invoke-Prepare $s } catch { $failed = $true; $message = $_.Exception.Message }
            Assert-True $failed 'install failure surfaces as error'
            Assert-True (-not $message.Contains('mismatch')) "failure came from install step: $message"
        } finally { $lock.Dispose() }
        Assert-True (@(Get-ChildItem -LiteralPath $s.ModelRoot -Force -Filter '.ocr-staging-*').Count -eq 0) 'staging cleaned after rollback'
        Assert-True ((Get-TreeSnapshot $s.ModelRoot) -ceq $before) 'det rolled back, rec untouched'
        Invoke-Prepare $s
        Assert-TargetValid $s
    }

    Test-Case 'missing manifest is rejected' {
        $s = New-Sandbox 'manifest-missing'
        Expect-Failure { Invoke-Prepare $s -Manifest (Join-Path $s.Root 'absent.json') } 'manifest file not found'
        Assert-True ($script:Downloads.Count -eq 0) 'no downloads'
        Assert-True (-not (Test-Path -LiteralPath $s.ModelRoot)) 'model root untouched'
    }

    Test-Case 'production function rejects synthetic manifest without a mock loader' {
        $s = New-Sandbox 'synthetic-rejected'
        Expect-Failure {
            Invoke-OcrModelPreparation -ManifestPath $s.Manifest -ModelRoot $s.ModelRoot -CacheRoot $s.CacheRoot -Downloader $fakeDownloader 6>$null
        } 'synthetic test manifest'
        Assert-True ($script:Downloads.Count -eq 0) 'no downloads'
    }

    Test-Case 'production entry: optional manifest and no synthetic switch' {
        $definition = (Get-Command Invoke-OcrModelPreparation).Definition
        Assert-True ($definition.Contains("[string]`$ManifestPath = ''")) 'manifest defaults to empty'
        Assert-True (-not $definition.Contains('Get-AssetManifestDefaultPath')) 'no implicit manifest dependency'
        $params = (Get-Command $scriptPath).Parameters.Keys
        Assert-True (-not ($params -contains 'AllowSynthetic') -and -not ($params -contains 'Downloader')) "script parameters: $($params -join ',')"
        Assert-True (-not (Get-Command Invoke-OcrModelPreparation).Parameters.ContainsKey('AllowSynthetic')) 'function exposes no synthetic switch'
    }

    Test-Case 'production entry exits non-zero for missing or synthetic manifest' {
        $s = New-Sandbox 'cli'
        $host_ = (Get-Process -Id $PID).Path
        $productionRoots = @(
            [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../src-tauri/resources/ocr-models')),
            [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../.ocr-model-cache'))
        )
        $before = @($productionRoots | ForEach-Object { Get-TreeSnapshot $_ })
        # 点源函数不指定 ModelRoot/CacheRoot，同样必须在写入前拒绝合成清单。
        Expect-Failure { Invoke-OcrModelPreparation -ManifestPath $s.Manifest -Downloader $fakeDownloader } 'synthetic test manifest'
        foreach ($case in @(
            @{ Args = @('-ManifestPath', (Join-Path $s.Root 'absent.json')); Expect = 'manifest file not found' },
            @{ Args = @('-ManifestPath', $s.Manifest); Expect = 'synthetic test manifest' },
            @{ Args = @('-ManifestPath', $s.Manifest, '-Profile', 'medium'); Expect = 'unsupported OCR model profile' }
        )) {
            $output = Invoke-ChildScript $host_ $case.Args
            Assert-True ($LASTEXITCODE -eq 1) "exit code 1 (got $LASTEXITCODE): $output"
            Assert-True ($output.Contains($case.Expect)) "output contains '$($case.Expect)': $output"
        }
        # 无参数成功路径由局部计划 mock + fake downloader 覆盖，绝不运行真实下载 CLI。
        for ($i = 0; $i -lt $productionRoots.Count; $i++) {
            Assert-True ((Get-TreeSnapshot $productionRoots[$i]) -ceq $before[$i]) "production target/cache unchanged: $($productionRoots[$i])"
        }
    }
} finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Test-PrepareOcrModels: $($script:Passed) cases passed ($($PSVersionTable.PSEdition) $($PSVersionTable.PSVersion))"
