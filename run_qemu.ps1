param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$BootImage,
    [ValidateSet("usb", "ps2")]
    [string]$KeyboardTransport = "usb",
    [ValidateSet("gtk", "sdl")]
    [string]$DisplayBackend = "gtk"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$qemuExe = "C:\Program Files\qemu\qemu-system-x86_64.exe"
$pwshExe = "C:\Program Files\PowerShell\7\pwsh.exe"
$serialWindowTitle = "OpenRhiza Serial Log"
$serialServerScript = Join-Path $repoRoot "tools\serial_log_server.ps1"

if (-not (Test-Path -LiteralPath $qemuExe)) {
    throw "QEMU executable not found at '$qemuExe'."
}

if (-not (Test-Path -LiteralPath $pwshExe)) {
    throw "PowerShell executable not found at '$pwshExe'."
}

if (-not (Test-Path -LiteralPath $serialServerScript)) {
    throw "Serial log server script not found at '$serialServerScript'."
}

$bootImagePath = (Resolve-Path -LiteralPath $BootImage).Path
$driverDisk = Join-Path $repoRoot "rhiza_drivers"

if (-not (Test-Path -LiteralPath $driverDisk)) {
    New-Item -ItemType Directory -Path $driverDisk | Out-Null
}

function Initialize-FixedCacheFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$Header,
        [Parameter(Mandatory = $true)]
        [int]$Size
    )

    $buffer = New-Object byte[] $Size

    if (Test-Path -LiteralPath $Path) {
        $existing = [System.IO.File]::ReadAllBytes($Path)
        $copyLength = [Math]::Min($existing.Length, $buffer.Length)
        [Array]::Copy($existing, 0, $buffer, 0, $copyLength)
    } else {
        $headerBytes = [System.Text.Encoding]::ASCII.GetBytes($Header)
        [Array]::Copy($headerBytes, 0, $buffer, 0, $headerBytes.Length)
    }

    if ($buffer[0] -eq 0) {
        $headerBytes = [System.Text.Encoding]::ASCII.GetBytes($Header)
        [Array]::Copy($headerBytes, 0, $buffer, 0, $headerBytes.Length)
    }

    [System.IO.File]::WriteAllBytes($Path, $buffer)
}

Initialize-FixedCacheFile -Path (Join-Path $driverDisk "DRVMAP.TXT") -Header "# OpenRhiza active driver map`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SKILLCCH.TXT") -Header "# OpenRhiza local skill cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SKLACTV.TXT") -Header "# OpenRhiza active skill map`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SOFTCCH.TXT") -Header "# OpenRhiza software cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "WORKCCH.TXT") -Header "# OpenRhiza workflow cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "POLICCH.TXT") -Header "# OpenRhiza policy cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "EVALCCH.TXT") -Header "# OpenRhiza evaluation cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SOFTCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=software`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SKILLCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=skills`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "WORKCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=workflows`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "POLICCH.TXT") -Header "# OpenRhiza capability cache`ndomain=policies`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "EVALCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=evaluations`nsummary=`n" -Size 512

foreach ($skillSlot in @("SK000.WAS", "SK001.WAS", "SK002.WAS", "SK003.WAS", "SK004.WAS", "SK005.WAS", "SK006.WAS", "SK007.WAS")) {
    Initialize-FixedCacheFile -Path (Join-Path $driverDisk $skillSlot) -Header "" -Size 65536
}

function Stop-OpenRhizaSession {
    Get-Process -Name "qemu-system-x86_64" -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue

    Get-NetTCPConnection -LocalPort 4444 -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique |
        ForEach-Object {
            Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue
        }

    Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowTitle -like "*$serialWindowTitle*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue

    Start-Sleep -Milliseconds 500
}

function Ensure-SerialLogWindow {
    $visibleWindow = Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowTitle -like "*$serialWindowTitle*" } |
        Select-Object -First 1

    $listenerReady = Get-NetTCPConnection -LocalPort 4444 -ErrorAction SilentlyContinue |
        Where-Object { $_.State -eq "Listen" } |
        Select-Object -First 1

    if ($listenerReady) {
        $listenerProcess = Get-Process -Id $listenerReady.OwningProcess -ErrorAction SilentlyContinue
        if ($listenerProcess -and $visibleWindow) {
            return
        }

        if ($listenerProcess) {
            Stop-Process -Id $listenerProcess.Id -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 500
        }
    }

    $cmdLine = "title $serialWindowTitle && `"$pwshExe`" -NoExit -File `"$serialServerScript`""
    Start-Process -FilePath "cmd.exe" -WorkingDirectory $repoRoot -ArgumentList @(
        "/k"
        $cmdLine
    ) -WindowStyle Normal | Out-Null

    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        Start-Sleep -Milliseconds 250
        $visibleWindow = Get-Process -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowTitle -like "*$serialWindowTitle*" } |
            Select-Object -First 1
        $listenerReady = Get-NetTCPConnection -LocalPort 4444 -ErrorAction SilentlyContinue |
            Where-Object { $_.State -eq "Listen" } |
            Select-Object -First 1
        if ($listenerReady -and $visibleWindow) {
            return
        }
    }

    throw "OpenRhiza serial log window failed to start."
}

Stop-OpenRhizaSession
Ensure-SerialLogWindow

$qemuArgs = @(
    "-no-reboot"
    "-no-shutdown"
    "-display", $DisplayBackend
    "-k", "en-us"
    "-drive", "format=raw,file=$bootImagePath"
    "-drive", "file=fat:rw:$driverDisk,format=raw,index=2"
    "-serial", "tcp:127.0.0.1:4444"
    "-monitor", "tcp:127.0.0.1:55555,server,nowait"
    "-netdev", "user,id=n1"
    "-device", "e1000,netdev=n1"
    "-device", "qemu-xhci,id=xhci"
)

if ($KeyboardTransport -eq "usb") {
    $qemuArgs += @(
        "-device", "usb-kbd,bus=xhci.0,port=1"
        "-device", "usb-mouse,bus=xhci.0,port=2"
    )
} else {
    $qemuArgs += @("-device", "usb-mouse,bus=xhci.0,port=1")
}

$qemu = Start-Process -FilePath $qemuExe -WorkingDirectory $repoRoot -ArgumentList $qemuArgs -PassThru
Wait-Process -Id $qemu.Id
