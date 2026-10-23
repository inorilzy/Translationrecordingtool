# 发布资产清单（scripts/release-assets.json）严格 schema 的 PowerShell 读取与校验模块。
#
# 本模块与 scripts/asset_manifest.py 实现同一协议（schemaVersion 1），协议说明见 Python 模块文档串；
# 任何一侧修改都必须同步另一侧，以及两种语言共享的用例 scripts/tests/fixtures/asset-manifest/。
#
# 为什么不用 ConvertFrom-Json：Windows PowerShell 5.1 与 PowerShell 7 对重复键、仅大小写不同的键、
# 日期样式字符串（会被自动转为 DateTime）、整数/浮点的处理各不相同，无法保证与 Python 侧结论一致。
# 因此这里内置一个小型严格 JSON 解析器：对象 → 按序且键大小写敏感的 OrderedDictionary，
# 数组 → List[object]，无小数/指数的整数 → Int64，其余数字 → Double，字符串保持原样。
#
# 本模块不联网、不下载、不写文件。用法：
#   Import-Module (Join-Path $PSScriptRoot 'AssetManifest.psm1') -Force
#   $manifest = Import-AssetManifest            # 默认读取 scripts/release-assets.json
#   $asset = Find-ApprovedAsset -Manifest $manifest -Kind 'ocr-model' -Selector @{ profile = 'small'; role = 'det' }

Set-StrictMode -Version 2.0

$script:SchemaVersion = 1
$script:DefaultManifestPath = Join-Path $PSScriptRoot 'release-assets.json'
$script:Kinds = @('ocr-model', 'ort-runtime', 'dictionary-input')
$script:ModelProfiles = @('tiny', 'small', 'medium')
$script:ModelRoles = @('det', 'rec')
$script:ModelFiles = @('inference.onnx', 'inference.json', 'inference.yml')
$script:OrtPlatforms = @('win')
$script:OrtArches = @('x64')
$script:DictionarySources = @('ecdict', 'wordnet')
$script:ApprovalStatuses = @('approved')
$script:EcdictRepo = 'skywind3000/ECDICT'
# 与 Python 侧一致的 size 上限（2^53-1）。
$script:MaxSize = [long]9007199254740991
$script:SelectorKeys = @{
    'ocr-model'        = @('profile', 'role')
    'ort-runtime'      = @('version', 'platform', 'arch')
    'dictionary-input' = @('source')
}

# 正则一律以 \z 结尾：.NET 的 $ 会匹配末尾换行之前的位置。默认区分大小写。
$script:ReSha256 = '^[0-9a-f]{64}\z'
$script:ReRevision = '^[0-9a-f]{40}\z'
$script:ReSemver = '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z'
$script:ReMajorMinor = '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z'
$script:ReDate = '^[0-9]{4}-[0-9]{2}-[0-9]{2}\z'

function Stop-AssetManifest {
    param([string]$Where, [string]$Message)
    # 统一异常类型，Test-AssetFile 只吞这一类，其他 I/O 异常照常抛出。
    throw [System.IO.InvalidDataException]::new("[asset-manifest] ${Where}: $Message")
}

# ---------------------------------------------------------------------------
# 严格 JSON 解析器
# ---------------------------------------------------------------------------

class AssetManifestJsonParser {
    [string]$Text
    [int]$Pos

    AssetManifestJsonParser([string]$text) {
        $this.Text = $text
        $this.Pos = 0
    }

    [void] Fail([string]$message) {
        throw [System.IO.InvalidDataException]::new("[asset-manifest] `$: invalid JSON ($message at offset $($this.Pos))")
    }

    [char] Peek() {
        if ($this.Pos -ge $this.Text.Length) { return [char]0 }
        return $this.Text[$this.Pos]
    }

    [void] SkipWhitespace() {
        # 与 JSON 规范及 Python json 一致，只认空格、\t、\n、\r。
        while ($this.Pos -lt $this.Text.Length) {
            $c = [int]$this.Text[$this.Pos]
            if ($c -eq 32 -or $c -eq 9 -or $c -eq 10 -or $c -eq 13) { $this.Pos++ } else { break }
        }
    }

    [object] ParseDocument() {
        $value = $this.ParseValue()
        $this.SkipWhitespace()
        if ($this.Pos -lt $this.Text.Length) { $this.Fail('extra data') }
        return $value
    }

    [bool] TryLiteral([string]$word) {
        if ($this.Pos + $word.Length -gt $this.Text.Length) { return $false }
        if ($this.Text.Substring($this.Pos, $word.Length) -cne $word) { return $false }
        $this.Pos += $word.Length
        return $true
    }

    [object] ParseValue() {
        $this.SkipWhitespace()
        if ($this.Pos -ge $this.Text.Length) { $this.Fail('unexpected end of input') }
        $c = $this.Peek()
        if ($c -ceq [char]'{') { return $this.ParseObject() }
        if ($c -ceq [char]'[') { return $this.ParseArray() }
        if ($c -ceq [char]'"') { return $this.ParseString() }
        if ($this.TryLiteral('true')) { return $true }
        if ($this.TryLiteral('false')) { return $false }
        if ($this.TryLiteral('null')) { return $null }
        if ($c -ceq [char]'-' -or ($c -ge [char]'0' -and $c -le [char]'9')) { return $this.ParseNumber() }
        $this.Fail('unexpected character')
        return $null
    }

    [object] ParseObject() {
        $this.Pos++
        $result = [System.Collections.Specialized.OrderedDictionary]::new([System.StringComparer]::Ordinal)
        # 与 Python 侧一致：同一对象内大小写不敏感的重复键直接拒绝。
        $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
        $this.SkipWhitespace()
        if ($this.Peek() -ceq [char]'}') {
            $this.Pos++
            return $result
        }
        while ($true) {
            $this.SkipWhitespace()
            if ($this.Peek() -cne [char]'"') { $this.Fail('expected object key') }
            $key = $this.ParseString()
            if (-not $seen.Add($key)) {
                throw [System.IO.InvalidDataException]::new("[asset-manifest] `$: duplicate key '$key'")
            }
            $this.SkipWhitespace()
            if ($this.Peek() -cne [char]':') { $this.Fail("expected ':'") }
            $this.Pos++
            $value = $this.ParseValue()
            $result.Add($key, $value)
            $this.SkipWhitespace()
            $next = $this.Peek()
            if ($next -ceq [char]',') { $this.Pos++; continue }
            if ($next -ceq [char]'}') { $this.Pos++; return $result }
            $this.Fail("expected ',' or '}'")
        }
        return $result
    }

    [object] ParseArray() {
        $this.Pos++
        $result = [System.Collections.Generic.List[object]]::new()
        $this.SkipWhitespace()
        if ($this.Peek() -ceq [char]']') {
            $this.Pos++
            return $result
        }
        while ($true) {
            $result.Add($this.ParseValue())
            $this.SkipWhitespace()
            $next = $this.Peek()
            if ($next -ceq [char]',') { $this.Pos++; continue }
            if ($next -ceq [char]']') { $this.Pos++; return $result }
            $this.Fail("expected ',' or ']'")
        }
        return $result
    }

    [string] ParseString() {
        $this.Pos++
        $builder = [System.Text.StringBuilder]::new()
        while ($true) {
            if ($this.Pos -ge $this.Text.Length) { $this.Fail('unterminated string') }
            $c = $this.Text[$this.Pos]
            if ($c -ceq [char]'"') {
                $this.Pos++
                return $builder.ToString()
            }
            if ([int]$c -lt 0x20) { $this.Fail('invalid control character in string') }
            if ($c -ceq [char]'\') {
                $this.Pos++
                if ($this.Pos -ge $this.Text.Length) { $this.Fail('unterminated escape') }
                $e = $this.Text[$this.Pos]
                if ($e -ceq [char]'"') { [void]$builder.Append([char]'"') }
                elseif ($e -ceq [char]'\') { [void]$builder.Append([char]'\') }
                elseif ($e -ceq [char]'/') { [void]$builder.Append([char]'/') }
                elseif ($e -ceq [char]'b') { [void]$builder.Append([char]8) }
                elseif ($e -ceq [char]'f') { [void]$builder.Append([char]12) }
                elseif ($e -ceq [char]'n') { [void]$builder.Append([char]10) }
                elseif ($e -ceq [char]'r') { [void]$builder.Append([char]13) }
                elseif ($e -ceq [char]'t') { [void]$builder.Append([char]9) }
                elseif ($e -ceq [char]'u') {
                    if ($this.Pos + 4 -ge $this.Text.Length) { $this.Fail('truncated unicode escape') }
                    $hex = $this.Text.Substring($this.Pos + 1, 4)
                    if ($hex -notmatch '^[0-9A-Fa-f]{4}\z') { $this.Fail('invalid unicode escape') }
                    [void]$builder.Append([char][System.Convert]::ToInt32($hex, 16))
                    $this.Pos += 4
                }
                else { $this.Fail('invalid escape') }
                $this.Pos++
                continue
            }
            [void]$builder.Append($c)
            $this.Pos++
        }
        return $builder.ToString()
    }

    [object] ParseNumber() {
        $match = [regex]::new('\G-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?').Match($this.Text, $this.Pos)
        if (-not $match.Success -or $match.Length -eq 0) { $this.Fail('invalid number') }
        $token = $match.Value
        $this.Pos += $match.Length
        $culture = [System.Globalization.CultureInfo]::InvariantCulture
        if (-not $match.Groups[2].Success -and -not $match.Groups[3].Success) {
            $integer = [long]0
            if ([long]::TryParse($token, [System.Globalization.NumberStyles]::AllowLeadingSign, $culture, [ref]$integer)) {
                return $integer
            }
        }
        # 小数、指数和超出 Int64 的整数都成为 Double，schema 中的整数字段会拒绝它们。
        return [double]::Parse($token, [System.Globalization.NumberStyles]::Float, $culture)
    }
}

# ---------------------------------------------------------------------------
# 基础断言（错误文本与 Python 实现保持一致）
# ---------------------------------------------------------------------------

function Test-IsManifestObject { param($Value) return ($Value -is [System.Collections.Specialized.OrderedDictionary]) }
function Test-IsManifestArray { param($Value) return ($Value -is [System.Collections.Generic.List[object]]) }

function Assert-ManifestObject {
    param($Value, [string]$Where, [string[]]$Required, [string[]]$Optional = @())
    if (-not (Test-IsManifestObject $Value)) { Stop-AssetManifest $Where 'must be an object' }
    foreach ($key in @($Value.Keys)) {
        if (-not ($Required -ccontains $key) -and -not ($Optional -ccontains $key)) {
            Stop-AssetManifest $Where "unknown field '$key'"
        }
    }
    foreach ($key in $Required) {
        if (-not $Value.Contains($key)) { Stop-AssetManifest $Where "missing field '$key'" }
    }
}

function Assert-ManifestString {
    param($Value, [string]$Where)
    if (-not ($Value -is [string])) { Stop-AssetManifest $Where 'must be a string' }
    if ([string]::IsNullOrWhiteSpace($Value)) { Stop-AssetManifest $Where 'must be a non-empty string' }
}

function Assert-ManifestPattern {
    param($Value, [string]$Where, [string]$Pattern, [string]$Description)
    Assert-ManifestString $Value $Where
    if (-not [regex]::IsMatch($Value, $Pattern)) { Stop-AssetManifest $Where "must match $Description" }
}

function Assert-ManifestEnum {
    param($Value, [string]$Where, [string[]]$Allowed)
    Assert-ManifestString $Value $Where
    if (-not ($Allowed -ccontains $Value)) { Stop-AssetManifest $Where ('must be one of: ' + ($Allowed -join ', ')) }
}

function Assert-ManifestEqual {
    param($Value, [string]$Where, [string]$Expected)
    Assert-ManifestString $Value $Where
    if ($Value -cne $Expected) { Stop-AssetManifest $Where "must equal '$Expected'" }
}

function Assert-ManifestPositiveInt {
    param($Value, [string]$Where)
    # 布尔、Double（含 1.0）与字符串都不是 Int64，一律拒绝。
    if (-not ($Value -is [long]) -or $Value -le 0 -or $Value -gt $script:MaxSize) {
        Stop-AssetManifest $Where 'must be a positive integer'
    }
}

# ---------------------------------------------------------------------------
# 结构校验
# ---------------------------------------------------------------------------

function New-ExpectedFile {
    param([string]$Name, [hashtable]$Locator, [string[]]$Order)
    $entry = [ordered]@{ name = $Name }
    foreach ($key in $Order) { $entry[$key] = $Locator[$key] }
    return $entry
}

# 按 kind 校验 identity，返回 @{ Id = 规范 id; Files = 期望文件列表 }。
function Get-ExpectedLayout {
    param([string]$Kind, $Identity, [string]$Where)

    if ($Kind -ceq 'ocr-model') {
        Assert-ManifestObject $Identity $Where @('profile', 'role', 'repo', 'revision')
        Assert-ManifestEnum $Identity['profile'] ($Where + '.profile') $script:ModelProfiles
        Assert-ManifestEnum $Identity['role'] ($Where + '.role') $script:ModelRoles
        $repo = 'PaddlePaddle/PP-OCRv6_' + $Identity['profile'] + '_' + $Identity['role'] + '_onnx'
        Assert-ManifestEqual $Identity['repo'] ($Where + '.repo') $repo
        Assert-ManifestPattern $Identity['revision'] ($Where + '.revision') $script:ReRevision '40-character lowercase hex commit'
        $revision = $Identity['revision']
        $files = @(foreach ($name in $script:ModelFiles) {
            New-ExpectedFile $name @{ url = "https://huggingface.co/$repo/resolve/$revision/$name" } @('url')
        })
        return @{ Id = ('ocr-model/' + $Identity['profile'] + '/' + $Identity['role']); Files = $files }
    }

    if ($Kind -ceq 'ort-runtime') {
        Assert-ManifestObject $Identity $Where @('version', 'platform', 'arch')
        Assert-ManifestPattern $Identity['version'] ($Where + '.version') $script:ReSemver 'MAJOR.MINOR.PATCH'
        Assert-ManifestEnum $Identity['platform'] ($Where + '.platform') $script:OrtPlatforms
        Assert-ManifestEnum $Identity['arch'] ($Where + '.arch') $script:OrtArches
        $version = $Identity['version']
        $package = 'onnxruntime-' + $Identity['platform'] + '-' + $Identity['arch'] + '-' + $version
        $files = @(
            (New-ExpectedFile "$package.zip" @{ url = "https://github.com/microsoft/onnxruntime/releases/download/v$version/$package.zip" } @('url')),
            (New-ExpectedFile 'onnxruntime.dll' @{ fromArchive = "$package.zip"; archivePath = "$package/lib/onnxruntime.dll" } @('fromArchive', 'archivePath'))
        )
        return @{ Id = ('ort-runtime/' + $version + '/' + $Identity['platform'] + '-' + $Identity['arch']); Files = $files }
    }

    # dictionary-input：ECDICT 与 WordNet 是两个独立输入，identity 形状按 source 区分。
    if (-not (Test-IsManifestObject $Identity)) { Stop-AssetManifest $Where 'must be an object' }
    if (-not $Identity.Contains('source')) { Stop-AssetManifest $Where "missing field 'source'" }
    Assert-ManifestEnum $Identity['source'] ($Where + '.source') $script:DictionarySources
    if ($Identity['source'] -ceq 'ecdict') {
        Assert-ManifestObject $Identity $Where @('source', 'repo', 'revision')
        Assert-ManifestEqual $Identity['repo'] ($Where + '.repo') $script:EcdictRepo
        Assert-ManifestPattern $Identity['revision'] ($Where + '.revision') $script:ReRevision '40-character lowercase hex commit'
        $url = 'https://raw.githubusercontent.com/' + $script:EcdictRepo + '/' + $Identity['revision'] + '/ecdict.csv'
        $files = @((New-ExpectedFile 'ecdict.csv' @{ url = $url } @('url')))
        return @{ Id = 'dictionary-input/ecdict'; Files = $files }
    }

    Assert-ManifestObject $Identity $Where @('source', 'version')
    Assert-ManifestPattern $Identity['version'] ($Where + '.version') $script:ReMajorMinor 'MAJOR.MINOR'
    $version = $Identity['version']
    $files = @((New-ExpectedFile "WNdb-$version.tar.gz" @{ url = "https://wordnetcode.princeton.edu/$version/WNdb-$version.tar.gz" } @('url')))
    return @{ Id = 'dictionary-input/wordnet'; Files = $files }
}

function Assert-ManifestFiles {
    param($Files, [string]$Where, [object[]]$Expected)
    if (-not (Test-IsManifestArray $Files)) { Stop-AssetManifest $Where 'must be an array' }
    $byName = [System.Collections.Generic.Dictionary[string, object]]::new([System.StringComparer]::Ordinal)
    foreach ($exp in $Expected) { $byName[$exp['name']] = $exp }
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    for ($i = 0; $i -lt $Files.Count; $i++) {
        $fw = $Where + '[' + $i + ']'
        $entry = $Files[$i]
        if (-not (Test-IsManifestObject $entry)) { Stop-AssetManifest $fw 'must be an object' }
        if (-not $entry.Contains('name')) { Stop-AssetManifest $fw "missing field 'name'" }
        $name = $entry['name']
        Assert-ManifestString $name ($fw + '.name')
        if (-not $byName.ContainsKey($name)) { Stop-AssetManifest ($fw + '.name') "unexpected file '$name'" }
        if (-not $seen.Add($name)) { Stop-AssetManifest ($fw + '.name') "duplicate file '$name'" }
        $exp = $byName[$name]
        $locator = @($exp.Keys | Where-Object { $_ -cne 'name' })
        Assert-ManifestObject $entry $fw (@('name') + $locator + @('size', 'sha256'))
        foreach ($key in $locator) { Assert-ManifestEqual $entry[$key] ($fw + '.' + $key) $exp[$key] }
        Assert-ManifestPositiveInt $entry['size'] ($fw + '.size')
        Assert-ManifestPattern $entry['sha256'] ($fw + '.sha256') $script:ReSha256 '64-character lowercase hex'
    }
    foreach ($exp in $Expected) {
        if (-not $seen.Contains($exp['name'])) { Stop-AssetManifest $Where ("missing required file '" + $exp['name'] + "'") }
    }
}

function Assert-ManifestApproval {
    param($Approval, [string]$Where)
    Assert-ManifestObject $Approval $Where @('status', 'approvedBy', 'approvedAt', 'method', 'evidence')
    Assert-ManifestEnum $Approval['status'] ($Where + '.status') $script:ApprovalStatuses
    Assert-ManifestString $Approval['approvedBy'] ($Where + '.approvedBy')
    $approvedAt = $Approval['approvedAt']
    Assert-ManifestPattern $approvedAt ($Where + '.approvedAt') $script:ReDate 'YYYY-MM-DD'
    $parsed = [datetime]::MinValue
    $ok = [datetime]::TryParseExact($approvedAt, 'yyyy-MM-dd', [System.Globalization.CultureInfo]::InvariantCulture,
        [System.Globalization.DateTimeStyles]::None, [ref]$parsed)
    if (-not $ok) { Stop-AssetManifest ($Where + '.approvedAt') 'must be a valid calendar date' }
    Assert-ManifestString $Approval['method'] ($Where + '.method')
    $evidence = $Approval['evidence']
    if (-not (Test-IsManifestArray $evidence)) { Stop-AssetManifest ($Where + '.evidence') 'must be an array' }
    if ($evidence.Count -eq 0) { Stop-AssetManifest ($Where + '.evidence') 'must not be empty' }
    for ($i = 0; $i -lt $evidence.Count; $i++) {
        Assert-ManifestString $evidence[$i] ($Where + '.evidence[' + $i + ']')
    }
}

function Assert-ManifestAsset {
    param($Asset, [string]$Where)
    Assert-ManifestObject $Asset $Where @('id', 'kind', 'identity', 'files', 'approval')
    $kind = $Asset['kind']
    Assert-ManifestString $kind ($Where + '.kind')
    if (-not ($script:Kinds -ccontains $kind)) { Stop-AssetManifest ($Where + '.kind') "unknown kind '$kind'" }
    $layout = Get-ExpectedLayout $kind $Asset['identity'] ($Where + '.identity')
    Assert-ManifestEqual $Asset['id'] ($Where + '.id') $layout.Id
    Assert-ManifestFiles $Asset['files'] ($Where + '.files') $layout.Files
    Assert-ManifestApproval $Asset['approval'] ($Where + '.approval')
}

# ---------------------------------------------------------------------------
# 导出接口
# ---------------------------------------------------------------------------

function Get-AssetManifestDefaultPath {
    <# 正式清单约定路径 scripts/release-assets.json（本模块不创建该文件）。 #>
    return $script:DefaultManifestPath
}

function ConvertFrom-AssetManifestJson {
    <# 严格解析 JSON 文本为 OrderedDictionary / List[object] 结构，不做 schema 校验。 #>
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ($Text.Length -gt 0 -and [int]$Text[0] -eq 0xFEFF) { $Text = $Text.Substring(1) }
    $value = [AssetManifestJsonParser]::new($Text).ParseDocument()
    # 逗号运算符阻止 PowerShell 管道展开顶层数组。
    return , $value
}

function Assert-AssetManifest {
    <# 校验已解析的清单；不合法抛出 InvalidDataException。生产调用方不得传 -AllowSynthetic。 #>
    param([Parameter(Mandatory = $true)][AllowNull()]$Manifest, [switch]$AllowSynthetic)
    Assert-ManifestObject $Manifest '$' @('schemaVersion', 'assets') @('synthetic')
    $schemaVersion = $Manifest['schemaVersion']
    if (-not ($schemaVersion -is [long]) -or $schemaVersion -ne $script:SchemaVersion) {
        Stop-AssetManifest '$.schemaVersion' "unsupported schemaVersion (expected $($script:SchemaVersion))"
    }
    if ($Manifest.Contains('synthetic')) {
        $synthetic = $Manifest['synthetic']
        if (-not ($synthetic -is [bool]) -or -not $synthetic) { Stop-AssetManifest '$.synthetic' 'must be true when present' }
        if (-not $AllowSynthetic) { Stop-AssetManifest '$.synthetic' 'synthetic test manifest is not trusted production data' }
    }
    $assets = $Manifest['assets']
    if (-not (Test-IsManifestArray $assets)) { Stop-AssetManifest '$.assets' 'must be an array' }
    if ($assets.Count -eq 0) { Stop-AssetManifest '$.assets' 'must not be empty' }
    $seenIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    for ($i = 0; $i -lt $assets.Count; $i++) {
        $where = '$.assets[' + $i + ']'
        Assert-ManifestAsset $assets[$i] $where
        $id = $assets[$i]['id']
        if (-not $seenIds.Add($id)) { Stop-AssetManifest ($where + '.id') "duplicate asset id '$id'" }
    }
}

function Import-AssetManifest {
    <# 读取并严格校验清单文件，返回清单（OrderedDictionary）。 #>
    param([string]$Path = $script:DefaultManifestPath, [switch]$AllowSynthetic)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { Stop-AssetManifest $Path 'manifest file not found' }
    $fullPath = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).ProviderPath
    $bytes = [System.IO.File]::ReadAllBytes($fullPath)
    $encoding = [System.Text.UTF8Encoding]::new($false, $true)
    $text = $null
    try {
        $text = $encoding.GetString($bytes)
    } catch {
        Stop-AssetManifest $Path 'invalid UTF-8'
    }
    $manifest = ConvertFrom-AssetManifestJson -Text $text
    Assert-AssetManifest -Manifest $manifest -AllowSynthetic:$AllowSynthetic
    return $manifest
}

function Find-ApprovedAsset {
    <#
    按选择键精确查找已批准条目；找不到即拒绝（未批准的 profile / 版本 / 架构 / override）。
    Selector 的键必须恰好是：ocr-model → profile, role；ort-runtime → version, platform, arch；
    dictionary-input → source。键名与值都区分大小写；别名（如 lite）须由调用方先归一。
    #>
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][hashtable]$Selector
    )
    if (-not ($script:Kinds -ccontains $Kind)) { Stop-AssetManifest 'selector' "unknown kind '$Kind'" }
    $keys = $script:SelectorKeys[$Kind]
    $exact = ($Selector.Count -eq $keys.Count)
    foreach ($key in @($Selector.Keys)) { if (-not ($keys -ccontains $key)) { $exact = $false } }
    if (-not $exact) { Stop-AssetManifest 'selector' ("kind '$Kind' requires exactly: " + ($keys -join ', ')) }
    foreach ($asset in $Manifest['assets']) {
        if ($asset['kind'] -cne $Kind) { continue }
        $match = $true
        foreach ($key in $keys) {
            $actual = $asset['identity'][$key]
            if (-not ($Selector[$key] -is [string]) -or $actual -cne $Selector[$key]) { $match = $false; break }
        }
        if ($match) { return $asset }
    }
    $label = ($keys | ForEach-Object { $_ + '=' + $Selector[$_] }) -join ', '
    Stop-AssetManifest '$.assets' "no approved asset for kind '$Kind' with $label"
}

function Get-AssetManifestFile {
    <# 返回条目中名为 Name 的文件记录（含 size / sha256，以及 url 或 fromArchive / archivePath）。 #>
    param([Parameter(Mandatory = $true)]$Asset, [Parameter(Mandatory = $true)][string]$Name)
    foreach ($entry in $Asset['files']) {
        if ($entry['name'] -ceq $Name) { return $entry }
    }
    Stop-AssetManifest ("asset '" + $Asset['id'] + "'") "no file '$Name'"
}

function Assert-AssetFile {
    <# 校验本地文件大小与 SHA-256，不符抛出 InvalidDataException。先比大小，避免对截断文件算摘要。 #>
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$FileEntry)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { Stop-AssetManifest $Path 'file not found' }
    $expectedSize = [long]$FileEntry['size']
    $actualSize = (Get-Item -LiteralPath $Path -ErrorAction Stop).Length
    if ($actualSize -ne $expectedSize) {
        Stop-AssetManifest $Path "size mismatch (expected $expectedSize, got $actualSize)"
    }
    $expectedHash = [string]$FileEntry['sha256']
    $actualHash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
    if ($actualHash -cne $expectedHash) {
        Stop-AssetManifest $Path "sha256 mismatch (expected $expectedHash, got $actualHash)"
    }
}

function Test-AssetFile {
    <# 缓存 / 目标复验用：缺失、大小或摘要不符返回 $false；其他 I/O 异常照常抛出。 #>
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$FileEntry)
    try {
        Assert-AssetFile -Path $Path -FileEntry $FileEntry
        return $true
    } catch [System.IO.InvalidDataException] {
        return $false
    }
}

Export-ModuleMember -Function @(
    'Get-AssetManifestDefaultPath',
    'ConvertFrom-AssetManifestJson',
    'Assert-AssetManifest',
    'Import-AssetManifest',
    'Find-ApprovedAsset',
    'Get-AssetManifestFile',
    'Assert-AssetFile',
    'Test-AssetFile'
)
