param(
    [string]$Elf = "target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf",
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-aarch64.exe",
    [ValidateSet("cortex-a53", "cortex-a72", "max")]
    [string]$Cpu = "cortex-a72",
    [int]$MemoryMiB = 1024,
    [double]$TimeoutSeconds = 10.0
)

$ErrorActionPreference = "Stop"

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$elfPath = (Resolve-Path (Join-Path $repo $Elf)).Path

if (-not (Test-Path -LiteralPath $Qemu)) {
    $candidate = Get-Command "qemu-system-aarch64.exe" -ErrorAction SilentlyContinue
    if ($candidate) {
        $Qemu = $candidate.Source
    } else {
        throw "QEMU not found: $Qemu"
    }
}

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()

$qemuArgs = @(
    "-M", "virt",
    "-cpu", $Cpu,
    "-m", "${MemoryMiB}M",
    "-kernel", $elfPath,
    "-device", "ramfb",
    "-display", "gtk",
    "-serial", "tcp:127.0.0.1:$port,server",
    "-monitor", "none",
    "-no-reboot"
)

$process = Start-Process -FilePath $Qemu -ArgumentList $qemuArgs -PassThru
$client = $null
$text = ""

try {
    $connectDeadline = (Get-Date).AddSeconds(5)
    while ((Get-Date) -lt $connectDeadline -and $null -eq $client) {
        try {
            $client = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $port)
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if ($null -eq $client) {
        throw "TCP serial connect timeout on port $port"
    }

    $stream = $client.GetStream()
    $stream.ReadTimeout = 200
    $stream.WriteTimeout = 1000
    $buffer = New-Object byte[] 4096
    $sent = $false
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)

    while ((Get-Date) -lt $deadline) {
        if ($process.HasExited) {
            throw "QEMU exited before GUI bootstrap completed"
        }

        try {
            $count = $stream.Read($buffer, 0, $buffer.Length)
            if ($count -gt 0) {
                $text += [Text.Encoding]::ASCII.GetString($buffer, 0, $count)
            }
        } catch [System.IO.IOException] {
        }

        if (-not $sent -and $text.Contains("input> ")) {
            $commands = "/display-status`r/skill-run skill_gui_session_bootstrap_v1`r"
            $bytes = [Text.Encoding]::ASCII.GetBytes($commands)
            $stream.Write($bytes, 0, $bytes.Length)
            $sent = $true
        }

        if ($sent -and $text.Contains("[Display] GUI/framebuffer policy is not compiled into ARM64 recovery core.")) {
            break
        }

        Start-Sleep -Milliseconds 50
    }
} finally {
    if ($client -ne $null) {
        $client.Close()
    }
}

$logPath = Join-Path $repo "target\arm64-gui-launch.log"
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $logPath) | Out-Null
Set-Content -LiteralPath $logPath -Value $text -Encoding UTF8

if (-not $text.Contains("[Display] GUI/framebuffer policy is not compiled into ARM64 recovery core.")) {
    throw "ARM64 GUI boundary marker not observed. See $logPath"
}

Write-Host "[ARM64 GUI] QEMU pid=$($process.Id) ramfb visible, but GUI rendering is denied in recovery core. Serial log: $logPath" -ForegroundColor Green
