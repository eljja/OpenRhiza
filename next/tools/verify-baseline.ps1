param([string]$LegacyRef)

$ErrorActionPreference = "Stop"
$nextRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$repoRoot = [IO.Path]::GetFullPath((Join-Path $nextRoot ".."))

function Read-Required([string]$RelativePath) {
    $path = Join-Path $repoRoot $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Missing required document: $RelativePath"
    }
    $text = Get-Content -LiteralPath $path -Raw -Encoding UTF8
    if ($text.Contains([char]0xFFFD)) { throw "Invalid text encoding: $RelativePath" }
    return $text
}

function Get-Matches([string]$Text, [string]$Pattern) {
    return @([regex]::Matches($Text, $Pattern) | ForEach-Object {
        $_.Value.ToLowerInvariant()
    } | Sort-Object -Unique) -join [char]10
}

$documents = @("next/README.md", "next/docs/DIRECTION.md", "next/docs/REQUIREMENTS.md",
    "next/docs/LEGACY_REVIEW.md", "next/docs/ROADMAP.md", "next/docs/IMPLEMENTATION_GATES.md",
    "next/docs/ASTRA_PLAN_REVIEW.md", "next/docs/ASTRA_REVIEW_DISPOSITION.md")
$texts = @{}
foreach ($document in $documents) {
    $korean = $document -replace '\.md$', '.ko.md'
    $en = Read-Required $document
    $ko = Read-Required $korean
    $texts[$document] = $en
    $texts[$korean] = $ko
    foreach ($pattern in @('(?m)^#{1,6} ', '(?m)^\|')) {
        if ([regex]::Matches($en, $pattern).Count -ne [regex]::Matches($ko, $pattern).Count) {
            throw "Translation structure mismatch: $document"
        }
    }
    foreach ($pattern in @('(?<![A-Za-z0-9])[RF]\d{2}(?![A-Za-z0-9])',
        '(?<![A-Za-z0-9])[0-9a-fA-F]{64}(?![A-Za-z0-9])', 'https?://[^)\s]+')) {
        if ((Get-Matches $en $pattern) -cne (Get-Matches $ko $pattern)) {
            throw "Translation identifier/source mismatch: $document"
        }
    }
}
foreach ($document in @("README.md", "README.en.md", "ARCHIVE.md", "CONTRIBUTING.md", "SECURITY.md", "AGENTS.md", "LICENSE")) {
    $texts[$document] = Read-Required $document
}

$links = 0
foreach ($document in $texts.Keys) {
    foreach ($match in [regex]::Matches($texts[$document], '\[[^\]]*\]\(([^)\r\n]+)\)')) {
        $target = $match.Groups[1].Value
        if ($target -match '^https?://' -or $target.StartsWith('#')) { continue }
        # Historical review evidence includes workstation paths and unpublished logs.
        if ($document -match '/ASTRA_PLAN_REVIEW(\.ko)?\.md$' -and
            $target -match '^([A-Za-z]:/|\.\./\.\./logs/)') { continue }
        $target = ($target -split '#', 2)[0]
        $absolute = [IO.Path]::GetFullPath((Join-Path (Split-Path (Join-Path $repoRoot $document)) $target))
        if (-not $absolute.StartsWith($repoRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
            -not (Test-Path -LiteralPath $absolute)) { throw "Broken local link: $document -> $target" }
        $links++
    }
}

foreach ($language in @("", ".ko")) {
    $requirements = $texts["next/docs/REQUIREMENTS$language.md"]
    $ids = @([regex]::Matches($requirements, '(?m)^\| (R\d{2}) \|') | ForEach-Object { $_.Groups[1].Value })
    $expected = @(1..25 | ForEach-Object { 'R{0:D2}' -f $_ })
    if (($ids -join ',') -cne ($expected -join ',')) { throw "Requirement rows mismatch: $language" }
    $review = $texts["next/docs/ASTRA_PLAN_REVIEW$language.md"]
    $ids = @([regex]::Matches($review, '(?m)^### (F\d{2}) -') | ForEach-Object { $_.Groups[1].Value })
    $expected = @(1..10 | ForEach-Object { 'F{0:D2}' -f $_ })
    if (($ids -join ',') -cne ($expected -join ',')) { throw "Finding sections mismatch: $language" }
}

$reference = (Read-Required "next/docs/legacy-reference.json") | ConvertFrom-Json
if ($reference.schema -ne "openrhiza.legacy.reference.v1" -or $reference.commit -notmatch '^[a-f0-9]{40}$' -or
    $reference.inventory_sha256 -notmatch '^[a-f0-9]{64}$' -or $reference.snapshot_file_count -lt 1) {
    throw "Invalid archive reference."
}
$inventoryPath = Join-Path $nextRoot "docs/legacy-inventory.json"
if ((Get-FileHash -LiteralPath $inventoryPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $reference.inventory_sha256) {
    throw "Frozen inventory hash mismatch."
}
$inventory = (Read-Required "next/docs/legacy-inventory.json") | ConvertFrom-Json
if ($inventory.schema -ne "openrhiza.legacy.inventory.v1" -or $inventory.file_count -ne $inventory.entries.Count -or
    $inventory.project_document_count -ne @($inventory.entries | Where-Object { $_.category -eq "project-document" }).Count) {
    throw "Invalid inventory schema or counts."
}
$seen = @{}
foreach ($entry in $inventory.entries) {
    if ($seen.ContainsKey($entry.path)) { throw "Duplicate inventory entry: $($entry.path)" }
    $seen[$entry.path] = $true
    if ($entry.path -notmatch '^[A-Za-z0-9_. /\[\]-]+$' -or $entry.path -match '(^/|(^|/)\.\.(/|$))' -or
        $entry.path -match '(^|/)(next|\.git|node_modules|\.next|target[^/]*|__pycache__|build|third_party|bootloader-patched)(/|$)' -or
        [IO.Path]::GetFileName($entry.path).StartsWith(".env") -or
        $entry.path -match '\.(pem|key|crt|wad|was|wasm|bin|img|pyc|o|exe|log)$' -or
        $entry.sha256 -notmatch '^[a-f0-9]{64}$' -or $entry.bytes -lt 0 -or $entry.lines -lt 0) {
        throw "Invalid or excluded inventory entry: $($entry.path)"
    }
}
foreach ($path in @("VISION.md", "OS.md", "src/main.rs", "src/core/seed.rs", "src/hangul.rs", "build.rs",
    "nic_drivers/driver_manifest.json", "platform-kernels/aarch64-recovery/src/main.rs",
    "openrhiza-nexus/src/app/registry-data.ts", "tools/semantic_graph_layer.py")) {
    if (-not $seen.ContainsKey($path)) { throw "Required review domain missing: $path" }
}

if ($LegacyRef) {
    $commit = (& git -C $repoRoot rev-parse --verify --end-of-options "$LegacyRef^{commit}") -join ""
    if ($LASTEXITCODE -ne 0 -or $commit -cne $reference.commit) { throw "Archive ref does not match pinned commit." }
    $paths = @(& git -C $repoRoot ls-tree -r --name-only $commit)
    if ($LASTEXITCODE -ne 0 -or $paths.Count -ne $reference.snapshot_file_count) { throw "Archive tree count mismatch." }
    $git = (Get-Command git -ErrorAction Stop).Source
    foreach ($entry in $inventory.entries) {
        $process = New-Object System.Diagnostics.Process
        $process.StartInfo.FileName = $git
        $process.StartInfo.WorkingDirectory = $repoRoot
        $process.StartInfo.Arguments = 'cat-file blob "' + $commit + ':' + $entry.path + '"'
        $process.StartInfo.UseShellExecute = $false
        $process.StartInfo.CreateNoWindow = $true
        $process.StartInfo.RedirectStandardOutput = $true
        $process.StartInfo.RedirectStandardError = $true
        $hasher = [Security.Cryptography.SHA256]::Create()
        try {
            $null = $process.Start()
            $actual = [BitConverter]::ToString($hasher.ComputeHash($process.StandardOutput.BaseStream)).Replace("-", "").ToLowerInvariant()
            $errorText = $process.StandardError.ReadToEnd()
            $process.WaitForExit()
            if ($process.ExitCode -ne 0 -or $actual -cne $entry.sha256) {
                throw "Archive content mismatch: $($entry.path). $errorText"
            }
        } finally {
            $hasher.Dispose()
            $process.Dispose()
        }
    }
    Write-Output "Pinned archive content verified: $($inventory.file_count) files at $commit."
} else {
    Write-Output "Archive content not read; use -LegacyRef after fetching the pinned archive branch."
}
Write-Output "Baseline document checks passed: 8 translation pairs, 25 requirements, 10 findings, $links local links. No OS/QEMU gate is implied."
