param(
    [switch]$SkipBuild,
    [switch]$VerboseLog,
    [ValidateRange(1, 1024)]
    [int]$Batches = 64,
    [string]$LogPath = ""
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

Push-Location $repoRoot
try {
    $args = @("-File", ".\tools\run_arm64_virtio_gate.ps1", "-OnlyLiveProductionRuntime", "-ProductionRuntimeBatches", $Batches.ToString())
    if ($SkipBuild) {
        $args += "-SkipBuild"
    }
    if ([string]::IsNullOrWhiteSpace($LogPath)) {
        $LogPath = Join-Path $repoRoot "target\arm64-production-runtime-demo.log"
    }
    $logDir = Split-Path -Parent $LogPath
    if (-not [string]::IsNullOrWhiteSpace($logDir)) {
        New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    }

    if ($VerboseLog) {
        & powershell.exe -ExecutionPolicy Bypass @args
        $exitCode = $LASTEXITCODE
    } else {
        Write-Host ("ARM64 production runtime demo: batches={0}" -f $Batches) -ForegroundColor Cyan
        Write-Host ("Writing full QEMU log to {0}" -f $LogPath) -ForegroundColor DarkGray
        $process = Start-Process -FilePath "powershell.exe" `
            -ArgumentList (@("-ExecutionPolicy", "Bypass") + $args) `
            -NoNewWindow `
            -Wait `
            -PassThru `
            -RedirectStandardOutput $LogPath `
            -RedirectStandardError ($LogPath + ".err")
        $exitCode = $process.ExitCode
        if (Test-Path ($LogPath + ".err")) {
            Get-Content ($LogPath + ".err") | Add-Content $LogPath
            Remove-Item ($LogPath + ".err") -Force
        }

        $summaryPatterns = @(
            "live OS pump counters",
            "production subsystem inbox",
            "production queue runtime",
            "virtio live service stop complete",
            "ARM64 virtio sandbox gates passed"
        )
        Write-Host ""
        Write-Host "Key evidence:" -ForegroundColor Cyan
        foreach ($pattern in $summaryPatterns) {
            $matches = Select-String -Path $LogPath -Pattern $pattern | Select-Object -Last 3
            foreach ($match in $matches) {
                Write-Host $match.Line
            }
        }
    }
    if ($exitCode -ne 0) {
        throw "ARM64 production runtime demo failed."
    }
} finally {
    Pop-Location
}
