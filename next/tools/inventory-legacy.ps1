param(
    [Parameter(Mandatory = $true)][string]$LegacyRoot,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = "Stop"
$nextRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$repoRoot = (Resolve-Path -LiteralPath $LegacyRoot -ErrorAction Stop).Path
$output = [IO.Path]::GetFullPath($OutputPath)
$frozenInventory = [IO.Path]::GetFullPath((Join-Path $nextRoot "docs/legacy-inventory.json"))
if ($output -eq $frozenInventory) { throw "The historical inventory is frozen; choose a separate output path." }
if (-not (Test-Path -LiteralPath (Join-Path $repoRoot "src/main.rs"))) { throw "LegacyRoot must be a legacy source checkout." }
$sourceRoots = @(
    "src/", "tools/", "sandbox-skills/", "sandbox-input-drivers/",
    "nic_drivers/", "platform-kernels/", "platforms/", "targets/",
    "filesystem-skills/", "semantic-graph/", "openrhiza-nexus/",
    ".cargo/", ".github/"
)
$extensions = @(".md", ".rs", ".py", ".ps1", ".ts", ".tsx", ".css", ".c", ".h", ".in", ".ld", ".json", ".toml", ".yml", ".yaml")

function Get-Category([string]$Path) {
    if ($Path.EndsWith(".md")) { return "project-document" }
    if ($Path.StartsWith("src/")) { return "legacy-kernel-and-services" }
    if ($Path.StartsWith("sandbox-skills/") -or $Path.StartsWith("sandbox-input-drivers/")) { return "sandbox-prototype" }
    if ($Path.StartsWith("nic_drivers/tests/")) { return "host-device-model-test" }
    if ($Path.StartsWith("nic_drivers/")) { return "device-candidate" }
    if ($Path.StartsWith("platform-kernels/")) { return "architecture-recovery-and-proof" }
    if ($Path.StartsWith("openrhiza-nexus/")) { return "legacy-nexus" }
    if ($Path.StartsWith("tools/") -or $Path.EndsWith(".py") -or $Path.EndsWith(".ps1")) { return "host-build-and-verification-tool" }
    return "build-platform-or-project-metadata"
}

Push-Location $repoRoot
try {
    $paths = @(& rg --files --hidden -g '!next/**' -g '!.git/**' -g '!**/node_modules/**' -g '!**/.next/**' -g '!**/target*/**' -g '!**/__pycache__/**' -g '!**/build/**' -g '!tools/fake-sysroot/**' -g '!third_party/**' -g '!bootloader-patched/**' -g '!logs/**' -g '!.tools/**' -g '!.fslab/**')
    if ($LASTEXITCODE -ne 0) { throw "Legacy inventory enumeration failed." }
    $entries = @(foreach ($rawPath in ($paths | Sort-Object -Unique)) {
        $path = $rawPath.Replace("\", "/")
        $inScope = -not $path.Contains("/")
        foreach ($root in $sourceRoots) {
            if ($path.StartsWith($root)) { $inScope = $true; break }
        }
        $extension = [IO.Path]::GetExtension($path).ToLowerInvariant()
        if (-not $inScope -or ($extensions -notcontains $extension -and $path -notin @("LICENSE", ".gitignore"))) { continue }
        if ([IO.Path]::GetFileName($path).StartsWith(".env")) { continue }

        $absolute = [IO.Path]::GetFullPath((Join-Path $repoRoot $path))
        if (-not $absolute.StartsWith($repoRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Inventory path escaped repository: $path"
        }
        $lineCount = 0
        foreach ($line in [IO.File]::ReadLines($absolute)) { $lineCount++ }
        [pscustomobject][ordered]@{
            path = $path
            category = Get-Category $path
            bytes = (Get-Item -LiteralPath $absolute).Length
            lines = $lineCount
            sha256 = (Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    })
    $branch = (& git branch --show-current) -join ""
    $commit = (& git rev-parse HEAD) -join ""
    if ($LASTEXITCODE -ne 0) { throw "Cannot identify the legacy Git reference." }
    $inventory = [ordered]@{
        schema = "openrhiza.legacy.inventory.v1"
        generated_at_utc = [DateTime]::UtcNow.ToString("o")
        branch = $branch
        commit = $commit
        includes_uncommitted_work = $true
        purpose = "Reference metadata and intent coverage, not a line-by-line security certification or new-runtime test."
        excludes = @("next implementation", "secrets and private keys", "generated images/binaries/data/logs", "dependency/build/cache trees", "bundled third-party and patched bootloader source")
        file_count = $entries.Count
        project_document_count = @($entries | Where-Object { $_.category -eq "project-document" }).Count
        entries = $entries
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $output) | Out-Null
    $inventory | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $output -Encoding UTF8
    Write-Output "Inventory: $($entries.Count) files, $($inventory.project_document_count) project documents."
    $entries | Group-Object category | Sort-Object Name | Select-Object Name, Count | Format-Table -AutoSize
} finally {
    Pop-Location
}
