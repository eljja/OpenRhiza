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
$pythonwExe = $null
$pythonExe = $null
$serialWindowTitle = "OpenRhiza Serial Log"
$serialServerScript = Join-Path $repoRoot "tools\serial_log_server.py"

if (-not (Test-Path -LiteralPath $qemuExe)) {
    throw "QEMU executable not found at '$qemuExe'."
}

function Resolve-PreferredPython {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CommandName
    )

    $wellKnownCandidates = @(
        "D:\python\anaconda3\$CommandName",
        "D:\python\anaconda3\Scripts\$CommandName"
    )
    foreach ($candidate in $wellKnownCandidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    $candidates = @(Get-Command $CommandName -All -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty Source -Unique)

    if (-not $candidates -or $candidates.Count -eq 0) {
        return $null
    }

    $preferred = $candidates | Where-Object {
        $_ -notmatch '\\WindowsApps\\' -and $_ -notmatch 'PythonSoftwareFoundation\.PythonManager'
    } | Select-Object -First 1

    if (-not [string]::IsNullOrWhiteSpace($preferred)) {
        return $preferred
    }

    return ($candidates | Select-Object -First 1)
}

$pythonwExe = Resolve-PreferredPython -CommandName "pythonw.exe"
$pythonExe = Resolve-PreferredPython -CommandName "python.exe"

if ([string]::IsNullOrWhiteSpace($pythonwExe) -and [string]::IsNullOrWhiteSpace($pythonExe)) {
    throw "Python executable not found."
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

function Initialize-FixedCacheFileFromText {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$Text,
        [Parameter(Mandatory = $true)]
        [int]$Size
    )

    $bytes = [System.Text.Encoding]::ASCII.GetBytes($Text)
    if ($bytes.Length -gt $Size) {
        throw "Content for $Path exceeds fixed cache size $Size bytes."
    }

    $buffer = New-Object byte[] $Size
    [Array]::Copy($bytes, $buffer, $bytes.Length)
    [System.IO.File]::WriteAllBytes($Path, $buffer)
}

function Merge-SkillCacheSeedMap {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $header = "# OpenRhiza local skill cache`n"
    $seedMap = [ordered]@{
        "skill_display_console_mode_v1" = "SK000.WAS"
        "skill_gui_session_bootstrap_v1" = "SK001.WAS"
        "skill_display_framebuffer_mode_v1" = "SK002.WAS"
        "skill_gui_compositor_seed_v1" = "SK003.WAS"
        "skill_registry_lookup_v1" = "SK004.WAS"
        "skill_gui_scene_mutator_v1" = "SK005.WAS"
    }

    $existingText = ""
    if (Test-Path -LiteralPath $Path) {
        $existingText = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($Path))
        $nulIndex = $existingText.IndexOf([char]0)
        if ($nulIndex -ge 0) {
            $existingText = $existingText.Substring(0, $nulIndex)
        }
    }

    $merged = [ordered]@{}
    foreach ($line in ($existingText -split "`r?`n")) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
            continue
        }
        $parts = $trimmed.Split("=", 2)
        if ($parts.Length -ne 2) {
            continue
        }
        $value = $parts[1].Trim()
        if (-not $value.EndsWith(".WAS")) {
            continue
        }
        $merged[$parts[0].Trim()] = $value
    }

    foreach ($key in $seedMap.Keys) {
        $merged[$key] = $seedMap[$key]
    }

    $text = $header
    foreach ($entry in $merged.GetEnumerator()) {
        $text += "$($entry.Key)=$($entry.Value)`n"
    }

    Initialize-FixedCacheFileFromText -Path $Path -Text $text -Size 512
}

function Install-SeedSkillSlot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourceName,
        [Parameter(Mandatory = $true)]
        [string]$TargetName,
        [Parameter(Mandatory = $true)]
        [int]$Size
    )

    $sourcePath = Join-Path $driverDisk $SourceName
    $targetPath = Join-Path $driverDisk $TargetName
    if (-not (Test-Path -LiteralPath $sourcePath)) {
        return
    }

    $sourceBytes = [System.IO.File]::ReadAllBytes($sourcePath)
    if ($sourceBytes.Length -gt $Size) {
        throw "$SourceName exceeds fixed skill slot size $Size bytes."
    }

    $buffer = New-Object byte[] $Size
    [Array]::Copy($sourceBytes, $buffer, $sourceBytes.Length)
    [System.IO.File]::WriteAllBytes($targetPath, $buffer)
}

Initialize-FixedCacheFile -Path (Join-Path $driverDisk "DRVMAP.TXT") -Header "# OpenRhiza active driver map`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SKILLCCH.TXT") -Header "# OpenRhiza local skill cache`n" -Size 512
Merge-SkillCacheSeedMap -Path (Join-Path $driverDisk "SKILLCCH.TXT")
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SKCAPCHE.TXT") -Header "# OpenRhiza capability cache`ndomain=skills`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SKLACTV.TXT") -Header "# OpenRhiza active skill map`n" -Size 512
if (Test-Path -LiteralPath (Join-Path $repoRoot "BOOT_AUTORUN.md")) {
    $bootAutorunText = Get-Content -LiteralPath (Join-Path $repoRoot "BOOT_AUTORUN.md") -Raw
    Initialize-FixedCacheFileFromText -Path (Join-Path $driverDisk "BOOTAUTO.MD") -Text $bootAutorunText -Size 2048
} else {
    Initialize-FixedCacheFile -Path (Join-Path $driverDisk "BOOTAUTO.MD") -Header "" -Size 2048
}
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SOFTCCH.TXT") -Header "# OpenRhiza software cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "WORKCCH.TXT") -Header "# OpenRhiza workflow cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "POLICCH.TXT") -Header "# OpenRhiza policy cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "EVALCCH.TXT") -Header "# OpenRhiza evaluation cache`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "SOFTCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=software`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "WORKCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=workflows`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "POLICCH.TXT") -Header "# OpenRhiza capability cache`ndomain=policies`nsummary=`n" -Size 512
Initialize-FixedCacheFile -Path (Join-Path $driverDisk "EVALCCH.TXT") -Header "# OpenRhiza capability cache`ndomain=evaluations`nsummary=`n" -Size 512

foreach ($skillSlot in @("SK000.WAS", "SK001.WAS", "SK002.WAS", "SK003.WAS", "SK004.WAS", "SK005.WAS", "SK006.WAS", "SK007.WAS")) {
    Initialize-FixedCacheFile -Path (Join-Path $driverDisk $skillSlot) -Header "" -Size 65536
}

Install-SeedSkillSlot -SourceName "SKDSP.WAS" -TargetName "SK000.WAS" -Size 65536
Install-SeedSkillSlot -SourceName "SKGUI.WAS" -TargetName "SK001.WAS" -Size 65536
Install-SeedSkillSlot -SourceName "SKFBUF.WAS" -TargetName "SK002.WAS" -Size 65536
Install-SeedSkillSlot -SourceName "SKCOMP.WAS" -TargetName "SK003.WAS" -Size 65536
Install-SeedSkillSlot -SourceName "SKREG.WAS" -TargetName "SK004.WAS" -Size 65536
Install-SeedSkillSlot -SourceName "SKMUT.WAS" -TargetName "SK005.WAS" -Size 65536

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

    $pythonLauncher = if (-not [string]::IsNullOrWhiteSpace($pythonwExe)) { $pythonwExe } else { $pythonExe }
    Start-Process -FilePath $pythonLauncher -WorkingDirectory $repoRoot -ArgumentList @(
        $serialServerScript
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

$resolvedDisplayBackend = switch ($DisplayBackend) {
    "gtk" { "gtk,grab-on-hover=on" }
    "sdl" { "sdl" }
    default { $DisplayBackend }
}

$qemuArgs = @(
    "-no-reboot"
    "-no-shutdown"
    "-display", $resolvedDisplayBackend
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

Push-Location $repoRoot
try {
    & $qemuExe @qemuArgs
} finally {
    Pop-Location
}
