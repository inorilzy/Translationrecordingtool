# 离线合成测试，无 Pester 依赖，不访问网络或用户缓存。
# powershell -NoProfile -File scripts/tests/Test-AssetManifest.ps1
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot '../AssetManifest.psm1') -Force
$fixtures = Join-Path $PSScriptRoot 'fixtures/asset-manifest'
$baseText = [IO.File]::ReadAllText((Join-Path $fixtures 'synthetic-valid.json'))
$cases = ConvertFrom-AssetManifestJson ([IO.File]::ReadAllText((Join-Path $fixtures 'cases.json')))
$count = 0
function Expect-Rejection {
    param([scriptblock]$Action, [string]$Expected)
    $rejected = $false
    try { & $Action | Out-Null } catch {
        $rejected = $true
        if (-not $_.Exception.Message.Contains($Expected)) { throw }
    }
    if (-not $rejected) { throw "Expected rejection: $Expected" }
}
Expect-Rejection { Import-AssetManifest (Join-Path $fixtures 'synthetic-valid.json') } 'synthetic test manifest'
$manifest = Import-AssetManifest (Join-Path $fixtures 'synthetic-valid.json') -AllowSynthetic
if ($manifest['assets'].Count -ne 5) { throw 'Incorrect asset count' }
foreach ($case in $cases['mutationCases']) {
    $data = ConvertFrom-AssetManifestJson $baseText
    foreach ($op in $case['ops']) {
        $parent = $data
        $path = $op['path']
        for ($i = 0; $i -lt $path.Count - 1; $i++) {
            $key = $path[$i]
            if ($key -is [long]) { $key = [int]$key }
            $parent = $parent[$key]
        }
        $key = $path[$path.Count - 1]
        if ($key -is [long]) { $key = [int]$key }
        if ($op['op'] -eq 'remove') {
            if ($parent -is [Collections.Generic.List[object]]) { $parent.RemoveAt($key) }
            else { $parent.Remove($key) }
        } elseif ($op['op'] -eq 'copy') {
            # 从另一份解析树取值，避免共享引用影响原条目。
            $value = ConvertFrom-AssetManifestJson $baseText
            foreach ($sourceKey in $op['from']) {
                $index = $sourceKey
                if ($index -is [long]) { $index = [int]$index }
                $value = $value[$index]
            }
            $parent[$key] = $value
        } else { $parent[$key] = $op['value'] }
    }
    try { Expect-Rejection { Assert-AssetManifest $data -AllowSynthetic } $case['expectError'] }
    catch { throw "Case '$($case['name'])': $_" }
    $count++
}
foreach ($case in $cases['rawCases']) {
    Expect-Rejection {
        $data = ConvertFrom-AssetManifestJson $case['text']
        Assert-AssetManifest $data -AllowSynthetic
    } $case['expectError']
    $count++
}
$asset = Find-ApprovedAsset $manifest 'ocr-model' @{ profile = 'small'; role = 'det' }
if ((Get-AssetManifestFile $asset 'inference.yml')['size'] -ne 103) { throw 'Incorrect file selection' }
Expect-Rejection { Find-ApprovedAsset $manifest 'ocr-model' @{ profile = 'tiny'; role = 'det' } } 'no approved asset'
Expect-Rejection { Find-ApprovedAsset $manifest 'ocr-model' @{ profile = 'small' } } 'requires exactly'
Expect-Rejection { Find-ApprovedAsset $manifest 'ort-runtime' @{ version = '9.0.0'; platform = 'win'; arch = 'x64' } } 'no approved asset'
# 临时文件默认使用系统临时盘；只删除本测试创建的文件。
$temp = [IO.Path]::GetTempFileName()
try {
    [IO.File]::WriteAllBytes($temp, [byte[]](97, 98, 99))
    $entry = @{ size = 3; sha256 = (Get-FileHash -LiteralPath $temp -Algorithm SHA256).Hash.ToLowerInvariant() }
    Assert-AssetFile $temp $entry
    [IO.File]::WriteAllBytes($temp, [byte[]](97, 98, 100))
    if (Test-AssetFile $temp $entry) { throw 'Corruption accepted' }
    [IO.File]::WriteAllBytes($temp, [byte[]](97, 98))
    if (Test-AssetFile $temp $entry) { throw 'Truncation accepted' }
    [IO.File]::WriteAllBytes($temp, [byte[]](255))
    Expect-Rejection { Import-AssetManifest $temp } 'invalid UTF-8'
} finally { [IO.File]::Delete($temp) }
if (Test-AssetFile $temp $entry) { throw 'Missing file accepted' }
Expect-Rejection { Import-AssetManifest $temp } 'manifest file not found'
Write-Output "PASS: $count shared rejection cases plus loading, selection and content checks"
