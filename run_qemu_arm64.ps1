param(
    [Parameter(Mandatory = $false, Position = 0)]
    [string]$KernelImage = "",
    [ValidateSet("cortex-a53", "cortex-a72", "max")]
    [string]$Cpu = "cortex-a72",
    [int]$MemoryMiB = 1024
)

$ErrorActionPreference = "Stop"

$qemuCandidates = @(
    "C:\Program Files\qemu\qemu-system-aarch64.exe",
    "qemu-system-aarch64.exe"
)

$qemuExe = $null
foreach ($candidate in $qemuCandidates) {
    $command = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($command) {
        $qemuExe = $command.Source
        break
    }
    if (Test-Path -LiteralPath $candidate) {
        $qemuExe = $candidate
        break
    }
}

if ([string]::IsNullOrWhiteSpace($qemuExe)) {
    throw "qemu-system-aarch64 was not found. Install QEMU with ARM64 system emulation before ARM bring-up."
}

if ([string]::IsNullOrWhiteSpace($KernelImage)) {
    Write-Host "OpenRhiza ARM64 runner is scaffolded, but no ARM64 kernel image exists yet." -ForegroundColor Yellow
    Write-Host "Next milestone: build an aarch64 serial recovery ELF, then run:" -ForegroundColor Yellow
    Write-Host "  pwsh -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 .\target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf"
    exit 0
}

$kernelPath = (Resolve-Path -LiteralPath $KernelImage).Path

$qemuArgs = @(
    "-machine", "virt",
    "-cpu", $Cpu,
    "-m", $MemoryMiB.ToString(),
    "-serial", "stdio",
    "-display", "gtk",
    "-no-reboot",
    "-kernel", $kernelPath
)

& $qemuExe @qemuArgs
