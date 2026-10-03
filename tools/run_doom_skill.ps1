param(
    [string]$BootImage = "target\x86_64-unknown-none\debug\bootimage-OpenRhiza.bin",
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe",
    [ValidateSet("whpx", "tcg")]
    [string]$Accelerator = "tcg",
    [double]$TimeoutSeconds = 90.0,
    [int]$TickCount = -1,
    [int]$TickDelayMilliseconds = 120,
    [string]$WadPath = "",
    [switch]$DownloadFreedoom,
    [switch]$DownloadDoomShareware,
    [switch]$UseDoomGeneric,
    [switch]$UseBootAutorunDoom,
    [string]$FreedoomCacheDir = "third_party\freedoom-cache",
    [string]$DoomSharewareCacheDir = "third_party\doom-shareware-cache",
    [switch]$Headless,
    [switch]$ExitAfterStart,
    [string[]]$PostStartCommands = @(),
    [int]$PostStartDelaySeconds = 0,
    [int]$RunSecondsAfterStart = 0,
    [switch]$ReleaseFrameBeforePostStart,
    [string]$LogPath = "logs\doom-skill-run.log"
)

$ErrorActionPreference = "Stop"

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$bootImagePath = (Resolve-Path (Join-Path $repo $BootImage)).Path
$driverDisk = Join-Path $repo "rhiza_drivers"

if (-not (Test-Path -LiteralPath $Qemu)) {
    throw "QEMU not found: $Qemu"
}
if (-not (Test-Path -LiteralPath $driverDisk)) {
    throw "Driver disk directory not found: $driverDisk"
}

function Set-DoomSkillCacheMap {
    param(
        [Parameter(Mandatory = $true)]
        [string]$DriverDisk
    )

    $path = Join-Path $DriverDisk "SKILLCCH.TXT"
    $header = "# OpenRhiza local skill cache`n"
    $capacity = 1536
    $entries = [ordered]@{}

    if (Test-Path -LiteralPath $path) {
        $existingBytes = [System.IO.File]::ReadAllBytes($path)
        $capacity = $existingBytes.Length
        $existingText = [Text.Encoding]::ASCII.GetString($existingBytes)
        $nulIndex = $existingText.IndexOf([char]0)
        if ($nulIndex -ge 0) {
            $existingText = $existingText.Substring(0, $nulIndex)
        }

        foreach ($line in ($existingText -split "`r?`n")) {
            $trimmed = $line.Trim()
            if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
                continue
            }
            $parts = $trimmed.Split("=", 2)
            if ($parts.Length -ne 2) {
                continue
            }
            $key = $parts[0].Trim()
            $value = $parts[1].Trim()
            if (-not [string]::IsNullOrWhiteSpace($key) -and $value.EndsWith(".WAS")) {
                $entries[$key] = $value
            }
        }
    }

    $entries["skill_doom_port_v1"] = "SKDOGEN.WAS"
    $text = $header
    foreach ($entry in $entries.GetEnumerator()) {
        $text += "$($entry.Key)=$($entry.Value)`n"
    }

    $bytes = [Text.Encoding]::ASCII.GetBytes($text)
    if ($bytes.Length -gt $capacity) {
        throw "Doom skill cache map exceeds fixed SKILLCCH.TXT capacity."
    }
    $buffer = New-Object byte[] $capacity
    [Array]::Copy($bytes, $buffer, $bytes.Length)
    [System.IO.File]::WriteAllBytes($path, $buffer)
}

Set-DoomSkillCacheMap -DriverDisk $driverDisk

$useDoomGenericRuntime = $true
$useAutorunDoom = $true
$bootAutorunPath = Join-Path $driverDisk "BOOTAUTO.MD"
$originalBootAutorunBytes = $null
if ($useAutorunDoom) {
    if (-not (Test-Path -LiteralPath $bootAutorunPath)) {
        throw "BOOTAUTO.MD not found for autorun Doom mode."
    }
    $originalBootAutorunBytes = [System.IO.File]::ReadAllBytes($bootAutorunPath)
    $bootAutorunText = [Text.Encoding]::ASCII.GetString($originalBootAutorunBytes)
    $nulIndex = $bootAutorunText.IndexOf([char]0)
    if ($nulIndex -ge 0) {
        $bootAutorunText = $bootAutorunText.Substring(0, $nulIndex)
    }
    if ($bootAutorunText -notmatch "/skill-load skill_doom_port_v1") {
        $doomAutorun = (
            "/skill-load skill_doom_port_v1",
            "@wait skill-stage skill_doom_port_v1 testing",
            "/skill-run skill_doom_port_v1"
        ) -join "`n"
        $bootAutorunText = $bootAutorunText -replace '```\s*$', ($doomAutorun + "`n" + '```')
    }
    $bytes = [Text.Encoding]::ASCII.GetBytes($bootAutorunText)
    if ($bytes.Length -gt 2048) {
        throw "BOOTAUTO.MD exceeds fixed 2048-byte capacity after adding Doom autorun."
    }
    $buffer = New-Object byte[] 2048
    [Array]::Copy($bytes, $buffer, $bytes.Length)
    [System.IO.File]::WriteAllBytes($bootAutorunPath, $buffer)
}

$normalizedPostStartCommands = New-Object System.Collections.Generic.List[string]
foreach ($entry in $PostStartCommands) {
    foreach ($part in ($entry -split ",")) {
        $trimmed = $part.Trim()
        if (-not [string]::IsNullOrWhiteSpace($trimmed)) {
            $normalizedPostStartCommands.Add($trimmed)
        }
    }
}
$PostStartCommands = $normalizedPostStartCommands.ToArray()

if ($TickCount -lt 0) {
    $TickCount = if ($Headless -or $ExitAfterStart) { 3 } else { 0 }
}

function Install-FreedoomWad {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CacheDir,
        [Parameter(Mandatory = $true)]
        [string]$DriverDisk
    )

    $cachePath = Join-Path $repo $CacheDir
    New-Item -ItemType Directory -Force -Path $cachePath | Out-Null

    $headers = @{
        "User-Agent" = "OpenRhiza-Doom-Skill"
        "Accept" = "application/vnd.github+json"
    }
    $release = Invoke-RestMethod `
        -Uri "https://api.github.com/repos/freedoom/freedoom/releases/latest" `
        -Headers $headers `
        -ErrorAction Stop
    $asset = $release.assets |
        Where-Object { $_.name -match '^freedoom-[0-9].*\.zip$' } |
        Select-Object -First 1
    if ($null -eq $asset) {
        throw "No FreeDoom release zip asset found in latest GitHub release."
    }

    $zipPath = Join-Path $cachePath $asset.name
    if (-not (Test-Path -LiteralPath $zipPath)) {
        Invoke-WebRequest `
            -Uri $asset.browser_download_url `
            -Headers @{ "User-Agent" = "OpenRhiza-Doom-Skill" } `
            -OutFile $zipPath `
            -ErrorAction Stop
    }

    $extractPath = Join-Path $cachePath ([IO.Path]::GetFileNameWithoutExtension($asset.name))
    if (-not (Test-Path -LiteralPath $extractPath)) {
        Expand-Archive -LiteralPath $zipPath -DestinationPath $extractPath -Force
    }

    $wad = Get-ChildItem -LiteralPath $extractPath -Recurse -File |
        Where-Object { $_.Name -ieq "freedoom1.wad" -or $_.Name -ieq "freedoom2.wad" } |
        Sort-Object Name |
        Select-Object -First 1
    if ($null -eq $wad) {
        throw "FreeDoom archive did not contain freedoom1.wad or freedoom2.wad."
    }

    Copy-Item -LiteralPath $wad.FullName -Destination (Join-Path $DriverDisk "FREEDOOM.WAD") -Force
    return $wad.FullName
}

function Install-DoomSharewareWad {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CacheDir,
        [Parameter(Mandatory = $true)]
        [string]$DriverDisk
    )

    $cachePath = Join-Path $repo $CacheDir
    New-Item -ItemType Directory -Force -Path $cachePath | Out-Null

    $packages = @(
        @{
            Name = "doom1.wad.zip"
            Uri = "https://www.jbserver.com/downloads/games/doom/misc/shareware/doom1.wad.zip"
        },
        @{
            Name = "doom19s.zip"
            Uri = "https://sourceforge.net/projects/dooms-19-winx64/files/packaged_zip/doom19s.zip/download"
        },
        @{
            Name = "doom19s-dosgames.zip"
            Uri = "https://www.dosgamesarchive.com/download/doom/doom19s"
        }
    )

    foreach ($package in $packages) {
        $zipPath = Join-Path $cachePath $package.Name
        if (-not (Test-Path -LiteralPath $zipPath)) {
            try {
                Invoke-WebRequest `
                    -Uri $package.Uri `
                    -Headers @{ "User-Agent" = "OpenRhiza-Doom-Skill" } `
                    -OutFile $zipPath `
                    -ErrorAction Stop
            } catch {
                Remove-Item -LiteralPath $zipPath -ErrorAction SilentlyContinue
                continue
            }
        }

        $extractPath = Join-Path $cachePath ([IO.Path]::GetFileNameWithoutExtension($package.Name))
        if (-not (Test-Path -LiteralPath $extractPath)) {
            try {
                Expand-Archive -LiteralPath $zipPath -DestinationPath $extractPath -Force
            } catch {
                continue
            }
        }

        $nestedZips = Get-ChildItem -LiteralPath $extractPath -Recurse -File -Filter "*.zip"
        foreach ($nestedZip in $nestedZips) {
            $nestedExtract = Join-Path $extractPath ([IO.Path]::GetFileNameWithoutExtension($nestedZip.Name))
            if (-not (Test-Path -LiteralPath $nestedExtract)) {
                Expand-Archive -LiteralPath $nestedZip.FullName -DestinationPath $nestedExtract -Force
            }
        }

        $wad = Get-ChildItem -LiteralPath $extractPath -Recurse -File |
            Where-Object { $_.Name -ieq "DOOM1.WAD" } |
            Select-Object -First 1
        if ($null -eq $wad) {
            continue
        }

        $sha1 = (Get-FileHash -LiteralPath $wad.FullName -Algorithm SHA1).Hash.ToLowerInvariant()
        if ($sha1 -ne "5b2e249b9c5133ec987b3ea77596381dc0d6bc1d") {
            continue
        }

        Copy-Item -LiteralPath $wad.FullName -Destination (Join-Path $DriverDisk "DOOM1.WAD") -Force
        return $wad.FullName
    }

    throw "Could not install verified Doom shareware DOOM1.WAD."
}

if ($DownloadDoomShareware -and $DownloadFreedoom) {
    throw "Use either -DownloadDoomShareware or -DownloadFreedoom, not both."
}

if ($DownloadDoomShareware) {
    if (-not [string]::IsNullOrWhiteSpace($WadPath)) {
        throw "Use either -DownloadDoomShareware or -WadPath, not both."
    }
    $installed = Install-DoomSharewareWad -CacheDir $DoomSharewareCacheDir -DriverDisk $driverDisk
    Write-Output "[doom] installed Doom shareware IWAD from $installed"
}

if ($DownloadFreedoom) {
    if (-not [string]::IsNullOrWhiteSpace($WadPath)) {
        throw "Use either -DownloadFreedoom or -WadPath, not both."
    }
    $installed = Install-FreedoomWad -CacheDir $FreedoomCacheDir -DriverDisk $driverDisk
    Write-Output "[doom] installed FreeDoom IWAD from $installed"
}

if (-not [string]::IsNullOrWhiteSpace($WadPath)) {
    $resolvedWad = (Resolve-Path -LiteralPath $WadPath).Path
    $targetName = Split-Path -Leaf $resolvedWad
    if ($targetName -notin @("DOOM1.WAD", "DOOM.WAD", "FREEDOOM.WAD")) {
        throw "WAD filename must be DOOM1.WAD, DOOM.WAD, or FREEDOOM.WAD for the bounded 8.3 root-file ABI."
    }
    Copy-Item -LiteralPath $resolvedWad -Destination (Join-Path $driverDisk $targetName) -Force
}

function Get-FreeTcpPort {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    return $port
}

function Send-HmpCommands {
    param(
        [Parameter(Mandatory = $true)]
        [int]$Port,
        [Parameter(Mandatory = $true)]
        [string[]]$Commands,
        [int]$PauseMilliseconds = 25
    )

    $client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $Port)
    try {
        $stream = $client.GetStream()
        $stream.ReadTimeout = 250
        $buffer = New-Object byte[] 4096
        try {
            [void]$stream.Read($buffer, 0, $buffer.Length)
        } catch [System.IO.IOException] {
        }

        foreach ($command in $Commands) {
            $bytes = [Text.Encoding]::ASCII.GetBytes($command + "`r`n")
            $stream.Write($bytes, 0, $bytes.Length)
            Start-Sleep -Milliseconds $PauseMilliseconds
        }
    } finally {
        $client.Close()
    }
}

function Convert-TextToSendKeyCommands {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text
    )

    $commands = New-Object System.Collections.Generic.List[string]
    foreach ($char in $Text.ToCharArray()) {
        $key = $null
        if ($char -ge 'a' -and $char -le 'z') {
            $key = [string]$char
        } elseif ($char -ge 'A' -and $char -le 'Z') {
            $key = "shift-" + ([string]$char).ToLowerInvariant()
        } elseif ($char -ge '0' -and $char -le '9') {
            $key = [string]$char
        } else {
            switch ($char) {
                "/" { $key = "slash" }
                "-" { $key = "minus" }
                "_" { $key = "shift-minus" }
                " " { $key = "spc" }
                default { throw "Cannot inject character through QEMU HMP: '$char'" }
            }
        }
        $commands.Add("sendkey $key")
    }
    return $commands.ToArray()
}

function Send-GuestCommand {
    param(
        [Parameter(Mandatory = $true)]
        [int]$MonitorPort,
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [switch]$ClearFirst
    )

    $commands = New-Object System.Collections.Generic.List[string]
    if ($ClearFirst) {
        for ($i = 0; $i -lt 16; $i++) {
            $commands.Add("sendkey backspace")
        }
    }
    foreach ($entry in (Convert-TextToSendKeyCommands -Text $Command)) {
        $commands.Add($entry)
    }
    $commands.Add("sendkey ret")
    Send-HmpCommands -Port $MonitorPort -Commands $commands.ToArray() -PauseMilliseconds 25
}

$serialPort = Get-FreeTcpPort
$monitorPort = Get-FreeTcpPort
$qemuAccelerator = if ($Accelerator -eq "tcg") { "tcg,thread=multi" } else { $Accelerator }
$qemuArgs = @(
    "-accel", $qemuAccelerator,
    "-no-reboot",
    "-no-shutdown",
    "-k", "en-us",
    "-drive", "file=$bootImagePath,format=raw,if=ide,index=0",
    "-drive", "file=fat:rw:$driverDisk,format=raw,if=ide,index=2",
    "-serial", "tcp:127.0.0.1:$serialPort,server",
    "-monitor", "tcp:127.0.0.1:$monitorPort,server,nowait",
    "-netdev", "user,id=n1",
    "-device", "e1000,netdev=n1",
    "-device", "qemu-xhci,id=xhci",
    "-device", "usb-mouse,bus=xhci.0,port=2"
)
if ($Headless) {
    $qemuArgs = @("-display", "none") + $qemuArgs
} else {
    $qemuArgs = @("-display", "gtk,show-cursor=on") + $qemuArgs
}

$windowStyle = if ($Headless) { "Hidden" } else { "Normal" }
$process = Start-Process -FilePath $Qemu -ArgumentList $qemuArgs -PassThru -WindowStyle $windowStyle
try {
    $process.PriorityClass = "High"
} catch {
    Write-Warning "Unable to raise QEMU process priority: $($_.Exception.Message)"
}
$client = $null
$text = ""
$sentDoomLoad = $useAutorunDoom
$sentDoom = $useAutorunDoom
$sentPostStart = $false
$completed = $false

try {
    $serialConnectTimeoutSeconds = [Math]::Min([Math]::Max([int]$TimeoutSeconds, 15), 45)
    $connectDeadline = (Get-Date).AddSeconds($serialConnectTimeoutSeconds)
    while ((Get-Date) -lt $connectDeadline -and $null -eq $client) {
        try {
            $client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $serialPort)
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if ($null -eq $client) {
        $exitHint = if ($process -ne $null -and $process.HasExited) {
            " QEMU exited with code $($process.ExitCode)."
        } else {
            ""
        }
        throw "TCP serial connect timeout after ${serialConnectTimeoutSeconds}s on port $serialPort.$exitHint"
    }

    $stream = $client.GetStream()
    $stream.ReadTimeout = 200
    $buffer = New-Object byte[] 4096
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)

    while ((Get-Date) -lt $deadline) {
        try {
            $count = $stream.Read($buffer, 0, $buffer.Length)
            if ($count -gt 0) {
                $text += [Text.Encoding]::ASCII.GetString($buffer, 0, $count)
            }
        } catch [System.IO.IOException] {
        }

        $guiReady = $text.Contains("[Display Runtime] GUI phase=sandbox-session") `
            -or $text.Contains("[Display Runtime] Validation=ready")
        if (-not $sentDoomLoad -and $guiReady) {
            Send-GuestCommand -MonitorPort $monitorPort -Command "/skill-load skill_doom_port_v1" -ClearFirst
            $sentDoomLoad = $true
            $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        }

        $doomLoaded = $text.Contains("[Skill Runtime] skill_doom_port_v1 stage=Testing") `
            -or $text.Contains("[Skill Runtime] skill_doom_port_v1 stage=Ready")
        if ($sentDoomLoad -and -not $sentDoom -and $doomLoaded) {
            Send-GuestCommand -MonitorPort $monitorPort -Command "/skill-run skill_doom_port_v1" -ClearFirst
            for ($i = 0; $i -lt $TickCount; $i++) {
                Start-Sleep -Milliseconds $TickDelayMilliseconds
                Send-GuestCommand -MonitorPort $monitorPort -Command "/skill-invoke skill_doom_port_v1 tick_skill" -ClearFirst
            }
            $sentDoom = $true
            $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        }

        $doomRuntimeStarted = $text.Contains("[Skill][doom] runtime started") `
            -or $text.Contains("[Skill][doomgeneric] original Doom renderer platform initialized.")
        $doomGraphicsReady = $text.Contains("I_InitGraphics: Auto-scaling factor")
        $doomRunReturned = $text.Contains("[Skill Runtime] skill_doom_port_v1 returned 1.") `
            -or ($useDoomGenericRuntime -and $doomLoaded -and $doomGraphicsReady)
        $doomTickReturned = $TickCount -eq 0 `
            -or $useDoomGenericRuntime `
            -or $text.Contains("[Skill Runtime] skill_doom_port_v1:tick_skill returned 1.")
        $doomStarted = $sentDoom -or ($useDoomGenericRuntime -and $doomLoaded)
        if ($doomStarted -and $doomRuntimeStarted -and $doomRunReturned -and $doomTickReturned) {
            if (-not $sentPostStart -and $PostStartCommands.Count -gt 0) {
                if ($PostStartDelaySeconds -gt 0) {
                    Start-Sleep -Seconds $PostStartDelaySeconds
                }
                if ($ReleaseFrameBeforePostStart) {
                    Send-HmpCommands -Port $monitorPort -Commands @("sendkey f12") -PauseMilliseconds 25
                    Start-Sleep -Milliseconds 750
                }
                foreach ($postCommand in $PostStartCommands) {
                    Send-GuestCommand -MonitorPort $monitorPort -Command $postCommand -ClearFirst
                    Start-Sleep -Milliseconds 500
                    try {
                        while ($stream.DataAvailable) {
                            $count = $stream.Read($buffer, 0, $buffer.Length)
                            if ($count -gt 0) {
                                $text += [Text.Encoding]::ASCII.GetString($buffer, 0, $count)
                            }
                        }
                    } catch [System.IO.IOException] {
                    }
                }
                $sentPostStart = $true
            }
            if ($RunSecondsAfterStart -gt 0) {
                $runDeadline = (Get-Date).AddSeconds($RunSecondsAfterStart)
                while ((Get-Date) -lt $runDeadline) {
                    try {
                        $count = $stream.Read($buffer, 0, $buffer.Length)
                        if ($count -gt 0) {
                            $text += [Text.Encoding]::ASCII.GetString($buffer, 0, $count)
                        }
                    } catch [System.IO.IOException] {
                    }
                }
            }
            $completed = $true
            break
        }

        Start-Sleep -Milliseconds 50
    }
} finally {
    if ($client -ne $null) {
        $client.Close()
    }

    if ($originalBootAutorunBytes -ne $null) {
        [System.IO.File]::WriteAllBytes($bootAutorunPath, $originalBootAutorunBytes)
    }

    $resolvedLogPath = Join-Path $repo $LogPath
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $resolvedLogPath) | Out-Null
    Set-Content -LiteralPath $resolvedLogPath -Value $text -Encoding UTF8

    if ($Headless -or $ExitAfterStart -or -not $completed) {
        if ($process -ne $null -and -not $process.HasExited) {
            Stop-Process -Id $process.Id -Force
        }
    }
}

if (-not $completed) {
    throw "Doom skill did not reach runnable state before timeout. See $LogPath"
}

Write-Output "[doom] skill_doom_port_v1 is running. log=$LogPath"
if (-not $Headless -and -not $ExitAfterStart) {
    Write-Output "[doom] QEMU window left open for interactive play."
}
