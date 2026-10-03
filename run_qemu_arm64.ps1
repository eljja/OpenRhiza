param(
    [Parameter(Mandatory = $false, Position = 0)]
    [string]$KernelImage = "",
    [switch]$NoGraphic,
    [switch]$Graphic,
    [switch]$Ramfb,
    [switch]$VirtioProbeDevice,
    [switch]$VirtioPayloadDevices,
    [ValidateSet("cortex-a53", "cortex-a72", "max")]
    [string]$Cpu = "cortex-a72",
    [int]$MemoryMiB = 1024
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

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
    $defaultKernel = Join-Path $repoRoot "target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf"
    if (Test-Path -LiteralPath $defaultKernel) {
        $KernelImage = $defaultKernel
    }
}

if ([string]::IsNullOrWhiteSpace($KernelImage)) {
    Write-Host "OpenRhiza ARM64 runner is scaffolded, but no ARM64 kernel image exists yet." -ForegroundColor Yellow
    Write-Host "Build it with:" -ForegroundColor Yellow
    Write-Host "  pwsh -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1"
    Write-Host "Next milestone: build an aarch64 serial recovery ELF, then run:" -ForegroundColor Yellow
    Write-Host "  pwsh -ExecutionPolicy Bypass -File .\run_qemu_arm64.ps1 .\target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf"
    exit 0
}

$kernelPath = (Resolve-Path -LiteralPath $KernelImage).Path

$qemuArgs = @(
    "-machine", "virt",
    "-cpu", $Cpu,
    "-m", $MemoryMiB.ToString(),
    "-no-reboot",
    "-kernel", $kernelPath
)

if ($VirtioProbeDevice -or $VirtioPayloadDevices) {
    $qemuArgs += @(
        "-netdev", "user,id=n1",
        "-device", "virtio-net-device,netdev=n1"
    )
}

if ($VirtioPayloadDevices) {
    $scratchImage = Join-Path $repoRoot "target\arm64-virtio-scratch.img"
    $scratchDir = Split-Path -Parent $scratchImage
    New-Item -ItemType Directory -Force -Path $scratchDir | Out-Null
    if (-not (Test-Path -LiteralPath $scratchImage)) {
        $stream = [System.IO.File]::Create($scratchImage)
        try {
            $stream.SetLength(8MB)
        } finally {
            $stream.Dispose()
        }
    }
    $qemuArgs += @(
        "-drive", "if=none,file=$scratchImage,format=raw,id=vd0",
        "-device", "virtio-blk-device,drive=vd0",
        "-device", "virtio-keyboard-device",
        "-device", "virtio-gpu-device"
    )
}

if ($Ramfb) {
    $qemuArgs += @("-device", "ramfb")
}

if ($NoGraphic -and $Graphic) {
    throw "Choose either -NoGraphic or -Graphic, not both."
}

if ($Graphic -or $Ramfb) {
    if ($Ramfb) {
        Write-Host "[ARM64 runner] RAMFB display backend enabled, but ARM64 recovery core no longer owns GUI rendering. Load a sandbox display/GUI skill after the display ABI exists." -ForegroundColor Cyan
    } else {
        Write-Host "[ARM64 runner] Graphic mode requested. Current ARM64 recovery output is serial-first; the QEMU display may stay inactive until a display skill owns a framebuffer." -ForegroundColor Yellow
    }
    $qemuArgs += @("-serial", "stdio", "-display", "gtk")
} else {
    Write-Host "[ARM64 runner] Serial recovery mode. Use -Graphic only for display/GPU experiments." -ForegroundColor Cyan
    $qemuArgs += @("-nographic")
}

& $qemuExe @qemuArgs
