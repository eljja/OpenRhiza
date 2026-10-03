param(
    [string]$Elf = "target\aarch64-openrhiza-none\debug\openrhiza-arm64.elf",
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-aarch64.exe",
    [double]$TimeoutSeconds = 8.0,
    [switch]$KeepLog,
    [string]$LogPath = "target\arm64-cli-tcp-smoke.log"
)

$ErrorActionPreference = "Stop"

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$elfPath = (Resolve-Path (Join-Path $repo $Elf)).Path

if (-not (Test-Path -LiteralPath $Qemu)) {
    throw "QEMU not found: $Qemu"
}

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()

$qemuArgs = @(
    "-M", "virt",
    "-cpu", "cortex-a57",
    "-m", "256M",
    "-kernel", $elfPath,
    "-display", "none",
    "-serial", "tcp:127.0.0.1:$port,server",
    "-monitor", "none"
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
        try {
            $count = $stream.Read($buffer, 0, $buffer.Length)
            if ($count -gt 0) {
                $text += [Text.Encoding]::ASCII.GetString($buffer, 0, $count)
            }
        } catch [System.IO.IOException] {
        }

        if (-not $sent -and $text.Contains("input> ")) {
            $commands = "/help`r/clear`r/status`r/registry-context`r/skill-cache`r/driver-host-status`r/display-status`r/input-routing-status`rcheck arm64 cli parity`r"
            $bytes = [Text.Encoding]::ASCII.GetBytes($commands)
            $stream.Write($bytes, 0, $bytes.Length)
            $sent = $true
        }

        if ($sent -and $text.Contains("[Prompt] internal console handoff is required")) {
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
    "OpenRhiza ARM64 recovery core",
    "input> ",
    "OpenRhiza ARM64 recovery CLI",
    "Local survival commands:",
    "[Registry] internal console handoff is present",
    "[Skill Cache] ARM64 recovery knows seed skill names only",
    "[ARM64 ABI] bounded platform identity probes only",
    "[Display] recovery surface=PL011 serial",
    "[Input] recovery input=PL011 serial",
    "[Prompt] internal console handoff is required"
)

foreach ($marker in $required) {
    if (-not $text.Contains($marker)) {
        throw "Missing ARM64 CLI smoke marker: $marker"
    }
}

if (-not $KeepLog) {
    Remove-Item -LiteralPath $resolvedLogPath -ErrorAction SilentlyContinue
}

Write-Output "[harness] ARM64 CLI TCP smoke passed"
