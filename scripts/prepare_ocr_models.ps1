# 默认按内置官方固定 revision/摘要准备 PP-OCRv6 small 模型，无需 release-assets.json 或人工审批。
# 显式 -ManifestPath 是自愿高级选项：沿用严格清单校验，缺失、非法、synthetic 或缺条目均拒绝。
#
# 完整性约束：
# - 内置摘要取自官方单渠道，属于完整性基线，不代表独立审批或多渠道验证。
# - 全部文件（inference.onnx / inference.json / inference.yml）按下载计划的固定 revision URL 获取；
#   缓存与已有目标逐文件校验大小与 SHA-256，不凭文件存在跳过。
# - 新内容先下载到临时文件、校验通过后才写入缓存；所有待装角色在暂存目录准备并复验后才替换目标，
#   任一失败都保留原目标。-Force 只是忽略已有缓存与目标强制重新下载，同样必须校验。
#
# 用法：powershell -ExecutionPolicy Bypass -File scripts/prepare_ocr_models.ps1 [-Profile small] [-Force] [-ManifestPath <path>]
# 测试 seam：点源（. prepare_ocr_models.ps1）时只定义函数、不执行；测试可向 Invoke-OcrModelPreparation
# 注入 -Downloader；合成清单仅由测试作用域的限定 mock loader 加载，生产入口与函数均拒绝。
[CmdletBinding()]
param(
    [string]$Profile = "small",
    [switch]$Force,
    [string]$ManifestPath
)

$ErrorActionPreference = "Stop"

Import-Module (Join-Path $PSScriptRoot "AssetManifest.psm1") -Force

$script:OcrRoles = @("det", "rec")
$script:OcrFiles = @("inference.onnx", "inference.json", "inference.yml")

function Write-OcrLog {
    param([string]$Message)
    Write-Host "[ocr-models] $Message"
}

function Resolve-OcrModelProfile {
    <# 按工单 21 决策正式只支持 small；tiny/medium 及其旧别名对应不同模型，不在批准范围内，一律拒绝。 #>
    param([string]$Name)
    if ($Name -ceq "small") { return "small" }
    throw "[ocr-models] unsupported OCR model profile '$Name' (only 'small' is supported)"
}

function Invoke-OcrDefaultDownload {
    param([string]$Url, [string]$OutFile)
    $previous = $ProgressPreference
    $ProgressPreference = "SilentlyContinue"
    try {
        Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing
    } finally {
        $ProgressPreference = $previous
    }
}

function Get-ApprovedOcrModelAsset {
    <# 取批准条目并复核身份与固定 revision URL（与清单 schema 双重校验，防止读取器变化后静默放宽）。 #>
    param($Manifest, [string]$ModelProfile, [string]$Role)
    $asset = Find-ApprovedAsset -Manifest $Manifest -Kind "ocr-model" -Selector @{ profile = $ModelProfile; role = $Role }
    $identity = $asset["identity"]
    $repo = "PaddlePaddle/PP-OCRv6_" + $ModelProfile + "_" + $Role + "_onnx"
    if ($identity["profile"] -cne $ModelProfile -or $identity["role"] -cne $Role -or $identity["repo"] -cne $repo) {
        throw "[ocr-models] asset '$($asset["id"])' identity does not match $ModelProfile/$Role ($repo)"
    }
    $revision = [string]$identity["revision"]
    if ($revision -cnotmatch '^[0-9a-f]{40}\z') {
        throw "[ocr-models] asset '$($asset["id"])' revision is not a fixed commit"
    }
    $files = @(foreach ($name in $script:OcrFiles) {
        $entry = Get-AssetManifestFile -Asset $asset -Name $name
        $expectedUrl = "https://huggingface.co/$repo/resolve/$revision/$name"
        if ([string]$entry["url"] -cne $expectedUrl) {
            throw "[ocr-models] asset '$($asset["id"])' file '$name' url is not pinned to revision $revision"
        }
        @{ Name = $name; Entry = $entry; Url = $expectedUrl }
    })
    return @{ Role = $Role; Repo = $repo; Revision = $revision; Files = $files }
}

function Get-DefaultOcrModelPlan {
    # 官方 Hugging Face 仓库单渠道 pins；不是 approval manifest，不声称独立审批。
    $pins = @{
        det = @{
            Revision = '28fe5895c24fd108c19eb3e8479f4ab385fbfc62'
            Sizes = @(9880512, 229307, 885)
            Hashes = @(
                'd73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e',
                '89240f689a4a77aad75ef55a8df0a15c8e1d4980a327d17e58f24bbadde5aeab',
                '193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516'
            )
        }
        rec = @{
            Revision = 'b8f84f0b80c529de40b4fbb3544b84fa7233a513'
            Sizes = @(21159378, 208004, 150579)
            Hashes = @(
                '5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634',
                'f0bf53c853937a917affdd74467472167727f8ab0f0f7bded01c4a16c27e46e6',
                'ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1'
            )
        }
    }
    foreach ($role in $script:OcrRoles) {
        $pin = $pins[$role]
        $repo = "PaddlePaddle/PP-OCRv6_small_${role}_onnx"
        $files = @(for ($i = 0; $i -lt $script:OcrFiles.Count; $i++) {
            $name = $script:OcrFiles[$i]
            $url = "https://huggingface.co/$repo/resolve/$($pin.Revision)/$name"
            @{ Name = $name; Url = $url; Entry = @{ name = $name; url = $url; size = $pin.Sizes[$i]; sha256 = $pin.Hashes[$i] } }
        })
        @{ Role = $role; Repo = $repo; Revision = $pin.Revision; Files = $files }
    }
}

function Test-OcrTargetReady {
    <# 目标目录中每个必需文件都必须与清单的大小和 SHA-256 一致。 #>
    param([string]$TargetDir, [object[]]$Files)
    if (-not (Test-Path -LiteralPath $TargetDir -PathType Container)) { return $false }
    foreach ($file in $Files) {
        if (-not (Test-AssetFile -Path (Join-Path $TargetDir $file.Name) -FileEntry $file.Entry)) { return $false }
    }
    return $true
}

function Get-VerifiedOcrCacheFile {
    <#
    返回已校验的缓存文件路径。缓存按 repo/revision 分目录；缓存缺失、截断、摘要不符或 -Force 时，
    下载到同目录临时文件，校验通过后才替换缓存文件；下载或校验失败时删除本次临时文件并抛错，原缓存不动。
    #>
    param([string]$CacheDir, $File, [scriptblock]$Downloader, [bool]$Refresh, [string]$Label)
    $cachePath = Join-Path $CacheDir $File.Name
    if (-not $Refresh -and (Test-AssetFile -Path $cachePath -FileEntry $File.Entry)) {
        Write-OcrLog "Using verified cache: $Label"
        return $cachePath
    }
    if (-not $Refresh -and (Test-Path -LiteralPath $cachePath)) {
        Write-OcrLog "Cached file failed verification, re-downloading: $Label"
    } else {
        Write-OcrLog "Downloading $Label"
    }
    New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
    $tempPath = Join-Path $CacheDir ($File.Name + ".download-" + [guid]::NewGuid().ToString("N"))
    try {
        & $Downloader $File.Url $tempPath
        Assert-AssetFile -Path $tempPath -FileEntry $File.Entry
        Move-Item -LiteralPath $tempPath -Destination $cachePath -Force
    } finally {
        if (Test-Path -LiteralPath $tempPath) { Remove-Item -LiteralPath $tempPath -Force }
    }
    Assert-AssetFile -Path $cachePath -FileEntry $File.Entry
    return $cachePath
}

function Install-OcrStagedRoles {
    <# 逐个以目录重命名替换目标；任一步失败则按相反顺序回滚已替换的角色，恢复原目标。 #>
    param([object[]]$Staged, [string]$WorkDir, [object[]]$Plans)
    $installed = New-Object System.Collections.Generic.List[object]
    try {
        foreach ($item in $Staged) {
            $backup = $null
            if (Test-Path -LiteralPath $item.Target) {
                $backup = Join-Path $WorkDir ($item.Role + ".previous")
                [System.IO.Directory]::Move($item.Target, $backup)
            }
            try {
                [System.IO.Directory]::Move($item.Stage, $item.Target)
            } catch {
                if ($backup) { [System.IO.Directory]::Move($backup, $item.Target) }
                throw
            }
            $installed.Add(@{ Item = $item; Backup = $backup })
        }
        # 最终目标复验同样属于事务；失败必须回滚。
        foreach ($plan in $Plans) {
            foreach ($file in $plan.Files) {
                Assert-AssetFile -Path (Join-Path $plan.Target $file.Name) -FileEntry $file.Entry
            }
        }
    } catch {
        for ($i = $installed.Count - 1; $i -ge 0; $i--) {
            $done = $installed[$i]
            [System.IO.Directory]::Move($done.Item.Target, (Join-Path $WorkDir ($done.Item.Role + ".rolled-back")))
            if ($done.Backup) { [System.IO.Directory]::Move($done.Backup, $done.Item.Target) }
        }
        throw
    }
}

function Invoke-OcrModelPreparation {
    [CmdletBinding()]
    param(
        [string]$Profile = "small",
        [switch]$Force,
        [string]$ManifestPath = '',
        [string]$ModelRoot = ([System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\src-tauri\resources\ocr-models"))),
        [string]$CacheRoot = ([System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.ocr-model-cache"))),
        [scriptblock]$Downloader = ${function:Invoke-OcrDefaultDownload}
    )

    # 所有拒绝判断（profile、清单、条目、URL）都在触碰文件系统之前完成。
    $modelProfile = Resolve-OcrModelProfile $Profile
    $plans = @(if ($ManifestPath) {
        $manifest = Import-AssetManifest -Path $ManifestPath
        foreach ($role in $script:OcrRoles) {
            Get-ApprovedOcrModelAsset -Manifest $manifest -ModelProfile $modelProfile -Role $role
        }
    } else {
        Get-DefaultOcrModelPlan
    })

    $profileRoot = Join-Path $ModelRoot $modelProfile
    $pending = @(foreach ($plan in $plans) {
        $targetDir = Join-Path $profileRoot $plan.Role
        if (-not $Force -and (Test-OcrTargetReady -TargetDir $targetDir -Files $plan.Files)) {
            Write-OcrLog "PP-OCRv6 $modelProfile/$($plan.Role) verified against pinned plan, skip. Use -Force to re-download."
            continue
        }
        $plan["Target"] = $targetDir
        $plan
    })
    if ($pending.Count -eq 0) {
        Write-OcrLog "Done (already verified): $profileRoot"
        return
    }

    New-Item -ItemType Directory -Force -Path $ModelRoot | Out-Null
    # 暂存目录与目标同卷（保证目录重命名可用），且位于 profile 目录之外，避免残留被资源通配符打包。
    $workDir = Join-Path $ModelRoot (".ocr-staging-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $workDir | Out-Null
    $completed = $false
    try {
        $staged = @(foreach ($plan in $pending) {
            $stageDir = Join-Path $workDir ($plan.Role + ".new")
            New-Item -ItemType Directory -Path $stageDir | Out-Null
            $cacheDir = Join-Path (Join-Path $CacheRoot ($plan.Repo -replace "[/\\:]", "_")) $plan.Revision
            foreach ($file in $plan.Files) {
                $label = "$($plan.Repo)@$($plan.Revision)/$($file.Name)"
                $cachePath = Get-VerifiedOcrCacheFile -CacheDir $cacheDir -File $file -Downloader $Downloader -Refresh ([bool]$Force) -Label $label
                $stagePath = Join-Path $stageDir $file.Name
                Copy-Item -LiteralPath $cachePath -Destination $stagePath -Force
                Assert-AssetFile -Path $stagePath -FileEntry $file.Entry
            }
            @{ Role = $plan.Role; Stage = $stageDir; Target = $plan.Target }
        })

        New-Item -ItemType Directory -Force -Path $profileRoot | Out-Null
        Install-OcrStagedRoles -Staged $staged -WorkDir $workDir -Plans $pending
        $completed = $true
    } finally {
        # 回滚自身或最终复验失败时，保留旧目标备份供人工恢复，绝不清除唯一旧副本。
        $backups = @(Get-ChildItem -LiteralPath $workDir -Filter '*.previous' -Directory -ErrorAction SilentlyContinue)
        if ($completed -or $backups.Count -eq 0) {
            if (Test-Path -LiteralPath $workDir) { Remove-Item -LiteralPath $workDir -Recurse -Force }
        } else {
            Write-Warning "[ocr-models] Recovery backup retained at $workDir"
        }
    }
    Write-OcrLog "Done: $profileRoot"
}

if ($MyInvocation.InvocationName -ne ".") {
    try {
        $arguments = @{ Profile = $Profile; Force = $Force }
        if ($ManifestPath) { $arguments["ManifestPath"] = $ManifestPath }
        Invoke-OcrModelPreparation @arguments
    } catch {
        [Console]::Error.WriteLine("[ocr-models] FAILED: " + $_.Exception.Message)
        exit 1
    }
}
