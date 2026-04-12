param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$BootImage
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$qemuExe = "C:\Program Files\qemu\qemu-system-x86_64.exe"

if (-not (Test-Path -LiteralPath $qemuExe)) {
    throw "QEMU executable not found at '$qemuExe'."
}

$bootImagePath = (Resolve-Path -LiteralPath $BootImage).Path
$driverDisk = Join-Path $repoRoot "rhiza_drivers"

$qemuArgs = @(
    "-no-reboot"
    "-no-shutdown"
    "-drive", "format=raw,file=$bootImagePath"
    "-drive", "file=fat:rw:$driverDisk,format=raw,index=2"
    "-serial", "tcp:127.0.0.1:4444,server,nowait"
    "-netdev", "user,id=n1"
    "-device", "e1000,netdev=n1"
    "-device", "qemu-xhci,id=xhci"
    "-device", "usb-kbd,bus=xhci.0"
)

$qemu = Start-Process -FilePath $qemuExe -WorkingDirectory $repoRoot -ArgumentList $qemuArgs -PassThru
Wait-Process -Id $qemu.Id
