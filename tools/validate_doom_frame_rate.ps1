param(
    [ValidateRange(60, 1000)]
    [int]$MinFps = 90,
    [ValidateRange(60, 1000)]
    [int]$MinFreshFps = 90,
    [int]$WarmupSamples = 0,
    [ValidateSet("whpx", "tcg")]
    [string]$Accelerator = "tcg",
    [int]$RunSecondsAfterStart = 20,
    [int]$TimeoutSeconds = 220,
    [string]$LogPath = "logs\doom-frame-rate-validation.log"
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

Push-Location $repo
try {
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\sandbox-skills\doomgeneric_openrhiza_seed\build.ps1"
    if ($LASTEXITCODE -ne 0) {
        throw "Doomgeneric skill build failed."
    }

    cargo bootimage
    if ($LASTEXITCODE -ne 0) {
        throw "bootimage build failed."
    }

    powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\tools\run_doom_skill.ps1" `
        -Headless `
        -ExitAfterStart `
        -TimeoutSeconds $TimeoutSeconds `
        -Accelerator $Accelerator `
        -DownloadDoomShareware `
        -RunSecondsAfterStart $RunSecondsAfterStart `
        -LogPath $LogPath
    if ($LASTEXITCODE -ne 0) {
        throw "Doom run failed."
    }

    $resolvedLog = Join-Path $repo $LogPath
    $floorWarnings = @(Select-String -LiteralPath $resolvedLog -SimpleMatch "[FrameResource][WARN]")
    if ($floorWarnings.Count -gt 0) {
        throw "Doom frame-resource runtime reported FPS floor violations. log=$LogPath"
    }

    $matches = Select-String -LiteralPath $resolvedLog -Pattern "\[FrameResource\].*fps=(\d+).*fresh_fps=(\d+)"
    $fpsValues = @()
    $freshValues = @()
    foreach ($match in $matches) {
        $fpsValues += [int]$match.Matches[0].Groups[1].Value
        $freshValues += [int]$match.Matches[0].Groups[2].Value
    }

    if ($fpsValues.Count -lt 4) {
        throw "Not enough FrameResource samples in $LogPath."
    }

    $steady = $fpsValues | Select-Object -Skip $WarmupSamples
    $freshSteady = $freshValues | Select-Object -Skip $WarmupSamples
    $below = @($steady | Where-Object { $_ -lt $MinFps })
    $freshBelow = @($freshSteady | Where-Object { $_ -lt $MinFreshFps })
    $last = $fpsValues[-1]
    $freshLast = $freshValues[-1]
    $minSteady = ($steady | Measure-Object -Minimum).Minimum
    $maxSteady = ($steady | Measure-Object -Maximum).Maximum
    $minFreshSteady = ($freshSteady | Measure-Object -Minimum).Minimum
    $maxFreshSteady = ($freshSteady | Measure-Object -Maximum).Maximum

    if ($below.Count -gt 0) {
        throw "Doom steady-state FPS below $MinFps. samples=$($fpsValues -join ',') log=$LogPath"
    }
    if ($MinFreshFps -gt 0 -and $freshBelow.Count -gt 0) {
        throw "Doom fresh-frame FPS below $MinFreshFps. fresh_samples=$($freshValues -join ',') log=$LogPath"
    }

    Write-Output "[doom-fps] pass min=$MinFps steady_min=$minSteady steady_max=$maxSteady last=$last fresh_min=$minFreshSteady fresh_max=$maxFreshSteady fresh_last=$freshLast samples=$($fpsValues -join ',') fresh_samples=$($freshValues -join ',') log=$LogPath"
} finally {
    Pop-Location
}
