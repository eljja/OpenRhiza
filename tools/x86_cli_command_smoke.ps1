param(
    [string]$BootImage = "target\x86_64-unknown-none\debug\bootimage-OpenRhiza.bin",
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe",
    [ValidateSet("whpx", "tcg")]
    [string]$Accelerator = "tcg",
    [string[]]$Commands = @("/status"),
    [string]$Expect = "[CLI] Keyboard input ready.",
    [double]$TimeoutSeconds = 60.0,
    [switch]$KeepLog,
    [string]$LogPath = "logs\x86-cli-command-smoke.log"
)

$ErrorActionPreference = "Stop"

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$bootImagePath = (Resolve-Path (Join-Path $repo $BootImage)).Path
$driverDisk = Join-Path $repo "rhiza_drivers"

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
        [int]$PauseMilliseconds = 35
    )
    $client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $Port)
    try {
        $stream = $client.GetStream()
        foreach ($command in $Commands) {
            $bytes = [Text.Encoding]::ASCII.GetBytes("$command`n")
            $stream.Write($bytes, 0, $bytes.Length)
            Start-Sleep -Milliseconds $PauseMilliseconds
        }
    } finally {
        $client.Close()
    }
}

function Convert-TextToSendKeyCommands {
    param([Parameter(Mandatory = $true)][string]$Text)
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
                ":" { $key = "shift-semicolon" }
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
        for ($i = 0; $i -lt 24; $i++) {
            $commands.Add("sendkey backspace")
        }
    }
    foreach ($entry in (Convert-TextToSendKeyCommands -Text $Command)) {
        $commands.Add($entry)
    }
    $commands.Add("sendkey ret")
    Send-HmpCommands -Port $MonitorPort -Commands $commands.ToArray()
}

$serialPort = Get-FreeTcpPort
$monitorPort = Get-FreeTcpPort
$qemuAccelerator = if ($Accelerator -eq "tcg") { "tcg,thread=multi" } else { $Accelerator }
$qemuArgs = @(
    "-accel", $qemuAccelerator,
    "-display", "none",
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
    "-device", "usb-kbd,bus=xhci.0,port=1",
    "-device", "usb-mouse,bus=xhci.0,port=2"
)

$process = Start-Process -FilePath $Qemu -ArgumentList $qemuArgs -PassThru -WindowStyle Hidden
$client = $null
$text = ""
$sent = $false
$completed = $false
try {
    $deadline = (Get-Date).AddSeconds([Math]::Min([Math]::Max([int]$TimeoutSeconds, 15), 45))
    while ((Get-Date) -lt $deadline) {
        try {
            $client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $serialPort)
            break
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if ($client -eq $null) {
        throw "TCP serial connect timeout on port $serialPort."
    }

    $stream = $client.GetStream()
    $buffer = New-Object byte[] 4096
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        while ($stream.DataAvailable) {
            $count = $stream.Read($buffer, 0, $buffer.Length)
            if ($count -gt 0) {
                $text += [Text.Encoding]::ASCII.GetString($buffer, 0, $count)
            }
        }

        if (-not $sent -and ($text.Contains("input>") -or $text.Contains("[Task] core_os_task started"))) {
            foreach ($command in $Commands) {
                Send-GuestCommand -MonitorPort $monitorPort -Command $command -ClearFirst
                Start-Sleep -Milliseconds 500
            }
            $sent = $true
        }

        if ($sent -and $text.Contains($Expect)) {
            $completed = $true
            break
        }
        Start-Sleep -Milliseconds 50
    }
} finally {
    if ($client -ne $null) {
        $client.Close()
    }
    $resolvedLogPath = Join-Path $repo $LogPath
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $resolvedLogPath) | Out-Null
    Set-Content -LiteralPath $resolvedLogPath -Value $text -Encoding UTF8
    if ($process -ne $null -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
    }
}

if (-not $completed) {
    throw "x86 CLI command smoke did not observe expected text '$Expect'. See $LogPath"
}

Write-Output "[harness] x86 CLI command smoke passed log=$LogPath"
if (-not $KeepLog) {
    Remove-Item -LiteralPath (Join-Path $repo $LogPath) -ErrorAction SilentlyContinue
}
