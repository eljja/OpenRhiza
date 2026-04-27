param()

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $repoRoot "rhiza_drivers"
$stripper = Join-Path $repoRoot "tools\strip_wasm_custom_sections.py"
$fixedSlotSize = 65536

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
    },
    @{
        Name = "display_framebuffer_mode"
        Root = Join-Path $repoRoot "sandbox-skills\display_framebuffer_mode"
        Artifact = "display_framebuffer_mode.wasm"
        FatName = "SKFBUF.WAS"
    },
    @{
        Name = "gui_compositor_seed"
        Root = Join-Path $repoRoot "sandbox-skills\gui_compositor_seed"
        Artifact = "gui_compositor_seed.wasm"
        FatName = "SKCOMP.WAS"
    },
    @{
        Name = "gui_scene_mutator_seed"
        Root = Join-Path $repoRoot "sandbox-skills\gui_scene_mutator_seed"
        Artifact = "gui_scene_mutator_seed.wasm"
        FatName = "SKMUT.WAS"
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
    & python $stripper $artifact $fatName
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to strip custom sections for $artifact"
    }
    Write-Host ("Built sandbox skill " + $target.Name + " -> " + $fatName)
}

function Write-FixedSkillSlot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourcePath,
        [Parameter(Mandatory = $true)]
        [string]$TargetPath,
        [Parameter(Mandatory = $true)]
        [int]$Size
    )

    $sourceBytes = [System.IO.File]::ReadAllBytes($SourcePath)
    if ($sourceBytes.Length -gt $Size) {
        throw ("Skill artifact exceeds fixed slot size: " + $SourcePath)
    }

    $buffer = New-Object byte[] $Size
    [Array]::Copy($sourceBytes, $buffer, $sourceBytes.Length)
    [System.IO.File]::WriteAllBytes($TargetPath, $buffer)
}

$slotMap = [ordered]@{
    "SKDSP.WAS" = "SK000.WAS"
    "SKGUI.WAS" = "SK001.WAS"
    "SKFBUF.WAS" = "SK002.WAS"
    "SKCOMP.WAS" = "SK003.WAS"
    "SKREG.WAS" = "SK004.WAS"
    "SKMUT.WAS" = "SK005.WAS"
}

foreach ($entry in $slotMap.GetEnumerator()) {
    $sourcePath = Join-Path $outDir $entry.Key
    $targetPath = Join-Path $outDir $entry.Value
    Write-FixedSkillSlot -SourcePath $sourcePath -TargetPath $targetPath -Size $fixedSlotSize
    Write-Host ("Seeded fixed slot " + $entry.Value + " from " + $entry.Key)
}
