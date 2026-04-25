param()

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $repoRoot "rhiza_drivers"

$targets = @(
    @{
        Name = "registry_lookup"
        Root = Join-Path $repoRoot "sandbox-skills\registry_lookup_bootstrap"
        Artifact = "registry_lookup_bootstrap.wasm"
        FatName = "SKREG.WAS"
    },
    @{
        Name = "display_console"
        Root = Join-Path $repoRoot "sandbox-skills\display_console_bootstrap"
        Artifact = "display_console_bootstrap.wasm"
        FatName = "SKDSP.WAS"
    },
    @{
        Name = "gui_session"
        Root = Join-Path $repoRoot "sandbox-skills\gui_session_bootstrap"
        Artifact = "gui_session_bootstrap.wasm"
        FatName = "SKGUI.WAS"
    }
)

foreach ($target in $targets) {
    Push-Location $target.Root
    try {
        cargo build --target wasm32-unknown-unknown --release
    } finally {
        Pop-Location
    }

    $artifact = Join-Path $target.Root ("target\wasm32-unknown-unknown\release\" + $target.Artifact)
    $fatName = Join-Path $outDir $target.FatName
    Copy-Item -LiteralPath $artifact -Destination $fatName -Force
    Write-Host ("Built sandbox skill " + $target.Name + " -> " + $fatName)
}
