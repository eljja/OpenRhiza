param(
    [string]$BootImage = "target\x86_64-unknown-none\debug\bootimage-OpenRhiza.bin",
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe",
    [ValidateSet("whpx", "tcg")]
    [string]$Accelerator = "tcg",
    [double]$TimeoutSeconds = 35.0,
    [switch]$KeepLog,
    [string]$LogPath = "logs\x86-gui-dynamic-smoke.log"
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
        for ($i = 0; $i -lt 12; $i++) {
            $commands.Add("sendkey backspace")
        }
    }
    foreach ($entry in (Convert-TextToSendKeyCommands -Text $Command)) {
        $commands.Add($entry)
    }
    $commands.Add("sendkey ret")
    Send-HmpCommands -Port $MonitorPort -Commands $commands.ToArray() -PauseMilliseconds 40
    Start-Sleep -Milliseconds 250
}

$serialPort = Get-FreeTcpPort
$monitorPort = Get-FreeTcpPort
$qemuAccelerator = if ($Accelerator -eq "tcg") { "tcg,thread=multi" } else { $Accelerator }
$qemuArgs = @(
    "-accel", $qemuAccelerator,
    "-no-reboot",
    "-no-shutdown",
    "-display", "none",
    "-k", "en-us",
    "-drive", "file=$bootImagePath,format=raw,if=ide,index=0",
    "-drive", "file=fat:rw:$driverDisk,format=raw,if=ide,index=2",
    "-serial", "tcp:127.0.0.1:$serialPort,server",
    "-monitor", "tcp:127.0.0.1:$monitorPort,server,nowait",
    "-netdev", "user,id=n1",
    "-device", "e1000,netdev=n1",
    "-device", "qemu-xhci,id=xhci",
    "-device", "usb-mouse,bus=xhci.0,port=1"
)

$process = Start-Process -FilePath $Qemu -ArgumentList $qemuArgs -PassThru -WindowStyle Hidden
$client = $null
$text = ""
$sentInitial = $false
$sentSceneAfterReady = $false
$sentReset = $false

try {
    $connectDeadline = (Get-Date).AddSeconds(8)
    while ((Get-Date) -lt $connectDeadline -and $null -eq $client) {
        try {
            $client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $serialPort)
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if ($null -eq $client) {
        throw "TCP serial connect timeout on port $serialPort"
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

        if (-not $sentInitial -and $text.Contains("[OS System] All subsystems initialized")) {
            Start-Sleep -Milliseconds 800
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-capabilities" -ClearFirst
            $sentInitial = $true
            $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        }

        if (
            $sentInitial `
            -and -not $sentSceneAfterReady `
            -and $text.Contains("[Display Runtime] GUI phase=sandbox-session") `
            -and $text.Contains("[Skill Runtime] skill_gui_modern_shell_v1 returned 1.")
        ) {
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-dynamic-demo" -ClearFirst
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-capabilities" -ClearFirst
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-scene" -ClearFirst
            $sentSceneAfterReady = $true
            $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        }

        if (
            $sentSceneAfterReady `
            -and -not $sentReset `
            -and $text.Contains("OpenRhiza Workbench") `
            -and $text.Contains("shell_policy_owner: skill_gui_modern_shell_v1") `
            -and $text.Contains("handle=131")
        ) {
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-reset all" -ClearFirst
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-capabilities" -ClearFirst
            Send-GuestCommand -MonitorPort $monitorPort -Command "/display-status" -ClearFirst
            Send-GuestCommand -MonitorPort $monitorPort -Command "/gui-scene" -ClearFirst
            $sentReset = $true
            $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        }

        $resetMarker = "[GUI] cleared all gui mutations and released shell policy owner"
        $resetIndex = $text.LastIndexOf($resetMarker)
        $afterReset = if ($resetIndex -ge 0) { $text.Substring($resetIndex) } else { "" }

        if (
            $sentReset `
            -and $text.Contains("GUI capabilities:") `
            -and $text.Contains("dynamic_handle_range: 120..159") `
            -and $text.Contains("[GUI] generic dynamic demo applied") `
            -and $text.Contains("handle=120") `
            -and $text.Contains("skill_gui_modern_shell_v1") `
            -and $afterReset.Contains("- shell_policy_owner: none") `
            -and $afterReset.Contains("label=Object 10") `
            -and $afterReset.Contains("label=Object 11")
        ) {
            break
        }

        Start-Sleep -Milliseconds 50
    }
} finally {
    if ($client -ne $null) {
        $client.Close()
    }
    if ($process -ne $null -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
    }
}

$resolvedLogPath = Join-Path $repo $LogPath
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $resolvedLogPath) | Out-Null
Set-Content -LiteralPath $resolvedLogPath -Value $text -Encoding UTF8

Write-Output $text

$required = @(
    "[OS System] All subsystems initialized",
    "GUI capabilities:",
    "dynamic_handle_range: 120..159",
    "[GUI] generic dynamic demo applied",
    "GUI scene:",
    "handle=120",
    "skill_gui_modern_shell_v1",
    "/gui-reset all",
    "[GUI] cleared all gui mutations and released shell policy owner"
)

foreach ($marker in $required) {
    if (-not $text.Contains($marker)) {
        throw "Missing x86 GUI dynamic smoke marker: $marker"
    }
}

$frameMatches = [regex]::Matches($text, "\[FrameResource\].*fps=(\d+).*fresh_fps=(\d+)")
if ($frameMatches.Count -gt 4) {
    $fpsValues = @()
    $freshValues = @()
    foreach ($match in $frameMatches) {
        $fpsValues += [int]$match.Groups[1].Value
        $freshValues += [int]$match.Groups[2].Value
    }
    $steady = $fpsValues | Select-Object -Skip 3
    $freshSteady = $freshValues | Select-Object -Skip 3
    $minFps = ($steady | Measure-Object -Minimum).Minimum
    $minFreshFps = ($freshSteady | Measure-Object -Minimum).Minimum
    if ($minFps -lt 60 -or $minFreshFps -lt 60) {
        throw "x86 GUI smoke frame floor below 60. fps_min=$minFps fresh_min=$minFreshFps"
    }
    Write-Output "[harness] frame floor passed fps_min=$minFps fresh_min=$minFreshFps"
}

$resetMarker = "[GUI] cleared all gui mutations and released shell policy owner"
$resetIndex = $text.LastIndexOf($resetMarker)
if ($resetIndex -lt 0) {
    throw "Missing x86 GUI rollback marker: $resetMarker"
}
$afterReset = $text.Substring($resetIndex)
$rollbackRequired = @(
    "- shell_policy_owner: none",
    "label=Object 10",
    "label=Object 11"
)
foreach ($marker in $rollbackRequired) {
    if (-not $afterReset.Contains($marker)) {
        throw "Missing x86 GUI rollback marker after reset: $marker"
    }
}

if (-not $KeepLog) {
    Remove-Item -LiteralPath $resolvedLogPath -ErrorAction SilentlyContinue
}

Write-Output "[harness] x86 GUI dynamic TCP smoke passed"
