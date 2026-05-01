param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$kernelRoot = Join-Path $repoRoot "platform-kernels\aarch64-recovery"
$targetSpec = Join-Path $repoRoot "targets\aarch64-openrhiza-none.json"
$targetDir = Join-Path $repoRoot "target\platform\aarch64-recovery"
$profile = if ($Release) { "release" } else { "debug" }
$outDir = Join-Path $repoRoot "target\aarch64-openrhiza-none\$profile"
$outElf = Join-Path $outDir "openrhiza-arm64.elf"

if (-not (Test-Path -LiteralPath $targetSpec)) {
    throw "Missing target spec: $targetSpec"
}

Push-Location $kernelRoot
try {
    $args = @(
        "build",
        "-Z", "json-target-spec",
        "-Z", "build-std=core,compiler_builtins",
        "--target", $targetSpec,
        "--target-dir", $targetDir
    )
    if ($Release) {
        $args += "--release"
    }
    cargo @args
    if ($LASTEXITCODE -ne 0) {
        throw "cargo failed while building ARM64 recovery ELF"
    }
} finally {
    Pop-Location
}

$builtElf = Join-Path $targetDir "aarch64-openrhiza-none\$profile\openrhiza-aarch64-recovery"
if (-not (Test-Path -LiteralPath $builtElf)) {
    throw "ARM64 recovery ELF was not produced at $builtElf"
}

New-Item -ItemType Directory -Force -Path $outDir | Out-Null
Copy-Item -LiteralPath $builtElf -Destination $outElf -Force
Write-Host "Created ARM64 recovery ELF: $outElf"
