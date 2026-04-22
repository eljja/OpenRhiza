param()

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $repoRoot "rhiza_drivers"

$targets = @(
    @{
        Name = "mouse"
        Root = Join-Path $repoRoot "sandbox-input-drivers\mouse_bootstrap"
        Artifact = "mouse_bootstrap_input_driver.wasm"
        FatName = "MOUSEDRV.WAS"
    },
    @{
        Name = "keyboard"
        Root = Join-Path $repoRoot "sandbox-input-drivers\keyboard_bootstrap"
        Artifact = "keyboard_bootstrap_input_driver.wasm"
        FatName = "KEYBDRV.WAS"
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
    Write-Host ("Built sandbox " + $target.Name + " input driver -> " + $fatName)
}
