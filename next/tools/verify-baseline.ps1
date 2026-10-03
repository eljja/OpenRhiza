param()

$ErrorActionPreference = "Stop"
$nextRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$repoRoot = [IO.Path]::GetFullPath((Join-Path $nextRoot ".."))
$inventory = Get-Content -Raw -LiteralPath (Join-Path $nextRoot "docs\legacy-inventory.json") | ConvertFrom-Json
if ($inventory.schema -ne "openrhiza.legacy.inventory.v1") { throw "Unexpected inventory schema." }
if ($inventory.file_count -ne $inventory.entries.Count) { throw "Inventory count mismatch." }

$seen = @{}
foreach ($entry in $inventory.entries) {
    if ($seen.ContainsKey($entry.path)) { throw "Duplicate inventory entry: $($entry.path)" }
    $seen[$entry.path] = $true
    if ($entry.path -match '(^|/)(next|\.git|node_modules|\.next|target[^/]*|__pycache__|build|third_party|bootloader-patched)(/|$)' -or
        [IO.Path]::GetFileName($entry.path).StartsWith(".env") -or
        $entry.path -match '\.(pem|key|crt|wad|was|wasm|bin|img|pyc|o|exe|log)$') {
        throw "Excluded content was inventoried: $($entry.path)"
    }
    $absolute = [IO.Path]::GetFullPath((Join-Path $repoRoot $entry.path))
    if (-not $absolute.StartsWith($repoRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Inventory path escaped repository."
    }
    $hash = (Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -ne $entry.sha256) { throw "Reference changed since inventory: $($entry.path)" }
}

foreach ($path in @("VISION.md", "OS.md", "src/main.rs", "src/core/seed.rs", "src/hangul.rs", "build.rs", "nic_drivers/driver_manifest.json", "platform-kernels/aarch64-recovery/src/main.rs", "openrhiza-nexus/src/app/registry-data.ts", "tools/semantic_graph_layer.py")) {
    if (-not $seen.ContainsKey($path)) { throw "Required review domain missing: $path" }
}
foreach ($path in @("README.md", "docs/DIRECTION.md", "docs/REQUIREMENTS.md", "docs/LEGACY_REVIEW.md", "docs/ROADMAP.md")) {
    if (-not (Test-Path -LiteralPath (Join-Path $nextRoot $path))) { throw "Missing new baseline document: $path" }
}
Write-Output "Reset baseline metadata checks passed; no new kernel/OS gate is implied."
