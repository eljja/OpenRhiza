param()

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$outDir = Join-Path $repoRoot "rhiza_drivers"
$stripper = Join-Path $repoRoot "tools\strip_wasm_custom_sections.py"
$fixedSlotSize = 65536
$pythonCandidates = @(
    "D:\Codex\.venv\Scripts\python.exe",
    "D:\python\anaconda3\python.exe",
    "python"
)
$python = $null
foreach ($candidate in $pythonCandidates) {
    $resolved = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($resolved) {
        $python = $resolved.Source
        break
    }
}
if (-not $python) {
    throw "No usable Python interpreter found for Wasm stripping"
}

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
    },
    @{
        Name = "fs_image_probe_bootstrap"
        Root = Join-Path $repoRoot "sandbox-skills\fs_image_probe_bootstrap"
        Artifact = "fs_image_probe_bootstrap.wasm"
        FatName = "SKFSP.WAS"
    },
    @{
        Name = "gui_modern_shell_seed"
        Root = Join-Path $repoRoot "sandbox-skills\gui_modern_shell_seed"
        Artifact = "gui_modern_shell_seed.wasm"
        FatName = "SKMSH.WAS"
    },
    @{
        Name = "desktop_compositor_seed"
        Root = Join-Path $repoRoot "sandbox-skills\desktop_compositor_seed"
        Artifact = "desktop_compositor_seed.wasm"
        FatName = "SKDSK.WAS"
    },
    @{
        Name = "qemu_driver_pack_seed"
        Root = Join-Path $repoRoot "sandbox-skills\qemu_driver_pack_seed"
        Artifact = "qemu_driver_pack_seed.wasm"
        FatName = "SKQDRV.WAS"
    },
    @{
        Name = "e1000_sandbox_probe_seed"
        Root = Join-Path $repoRoot "sandbox-skills\e1000_sandbox_probe_seed"
        Artifact = "e1000_sandbox_probe_seed.wasm"
        FatName = "SKE1000.WAS"
    },
    @{
        Name = "storage_host_probe_seed"
        Root = Join-Path $repoRoot "sandbox-skills\storage_host_probe_seed"
        Artifact = "storage_host_probe_seed.wasm"
        FatName = "SKSTOR.WAS"
    },
    @{
        Name = "piix_ide_sandbox_probe_seed"
        Root = Join-Path $repoRoot "sandbox-skills\piix_ide_sandbox_probe_seed"
        Artifact = "piix_ide_sandbox_probe_seed.wasm"
        FatName = "SKIDE.WAS"
    },
    @{
        Name = "xhci_sandbox_probe_seed"
        Root = Join-Path $repoRoot "sandbox-skills\xhci_sandbox_probe_seed"
        Artifact = "xhci_sandbox_probe_seed.wasm"
        FatName = "SKXHCI.WAS"
    },
    @{
        Name = "voice_capture_bridge_seed"
        Root = Join-Path $repoRoot "sandbox-skills\voice_capture_bridge_seed"
        Artifact = "voice_capture_bridge_seed.wasm"
        FatName = "SKVOICE.WAS"
    },
    @{
        Name = "text_web_browser_seed"
        Root = Join-Path $repoRoot "sandbox-skills\text_web_browser_seed"
        Artifact = "text_web_browser_seed.wasm"
        FatName = "SKTWEB.WAS"
    },
    @{
        Name = "gui_web_browser_seed"
        Root = Join-Path $repoRoot "sandbox-skills\gui_web_browser_seed"
        Artifact = "gui_web_browser_seed.wasm"
        FatName = "SKGWEB.WAS"
    },
    @{
        Name = "network_diagnostics_seed"
        Root = Join-Path $repoRoot "sandbox-skills\network_diagnostics_seed"
        Artifact = "network_diagnostics_seed.wasm"
        FatName = "SKNDIAG.WAS"
    },
    @{
        Name = "system_monitor_seed"
        Root = Join-Path $repoRoot "sandbox-skills\system_monitor_seed"
        Artifact = "system_monitor_seed.wasm"
        FatName = "SKSYSM.WAS"
    },
    @{
        Name = "virtio_mmio_probe_seed"
        Root = Join-Path $repoRoot "sandbox-skills\virtio_mmio_probe_seed"
        Artifact = "virtio_mmio_probe_seed.wasm"
        FatName = "SKVIRT.WAS"
    },
    @{
        Name = "virtio_net_driver_seed"
        Root = Join-Path $repoRoot "sandbox-skills\virtio_net_driver_seed"
        Artifact = "virtio_net_driver_seed.wasm"
        FatName = "SKVNET.WAS"
    },
    @{
        Name = "virtio_block_driver_seed"
        Root = Join-Path $repoRoot "sandbox-skills\virtio_block_driver_seed"
        Artifact = "virtio_block_driver_seed.wasm"
        FatName = "SKVBLK.WAS"
    },
    @{
        Name = "virtio_input_driver_seed"
        Root = Join-Path $repoRoot "sandbox-skills\virtio_input_driver_seed"
        Artifact = "virtio_input_driver_seed.wasm"
        FatName = "SKVINP.WAS"
    },
    @{
        Name = "virtio_gpu_driver_seed"
        Root = Join-Path $repoRoot "sandbox-skills\virtio_gpu_driver_seed"
        Artifact = "virtio_gpu_driver_seed.wasm"
        FatName = "SKVGPU.WAS"
    },
    @{
        Name = "arm64_virtio_gpu_payload_seed"
        Root = Join-Path $repoRoot "sandbox-skills\arm64_virtio_gpu_payload_seed"
        Artifact = "arm64_virtio_gpu_payload_seed.wasm"
        FatName = "SKVAGP.WAS"
    },
    @{
        Name = "arm64_wasm_log_probe_seed"
        Root = Join-Path $repoRoot "sandbox-skills\arm64_wasm_log_probe_seed"
        Artifact = "arm64_wasm_log_probe_seed.wasm"
        FatName = "SKWRUN.WAS"
    },
    @{
        Name = "doom_port_seed"
        Root = Join-Path $repoRoot "sandbox-skills\doom_port_seed"
        Artifact = "doom_port_seed.wasm"
        FatName = "SKDOOM.WAS"
    },
    @{
        Name = "keyboard_bootstrap_input_driver"
        Root = Join-Path $repoRoot "sandbox-input-drivers\keyboard_bootstrap"
        Artifact = "keyboard_bootstrap_input_driver.wasm"
        FatName = "KEYBDRV.WAS"
    },
    @{
        Name = "mouse_bootstrap_input_driver"
        Root = Join-Path $repoRoot "sandbox-input-drivers\mouse_bootstrap"
        Artifact = "mouse_bootstrap_input_driver.wasm"
        FatName = "MOUSEDRV.WAS"
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
    & $python $stripper $artifact $fatName
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to strip custom sections for $artifact"
    }
    Write-Host ("Built sandbox skill " + $target.Name + " -> " + $fatName)
}

$doomGenericBuild = Join-Path $repoRoot "sandbox-skills\doomgeneric_openrhiza_seed\build.ps1"
if (Test-Path -LiteralPath $doomGenericBuild) {
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File $doomGenericBuild
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build doomgeneric OpenRhiza skill"
    }
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
    "SKFSP.WAS" = "SK006.WAS"
    "SKMSH.WAS" = "SK007.WAS"
    "SKQDRV.WAS" = "SK008.WAS"
    "SKVOICE.WAS" = "SK009.WAS"
    "SKE1000.WAS" = "SK010.WAS"
    "SKSTOR.WAS" = "SK011.WAS"
    "SKIDE.WAS" = "SK012.WAS"
    "SKXHCI.WAS" = "SK013.WAS"
    "SKTWEB.WAS" = "SK014.WAS"
    "SKGWEB.WAS" = "SK015.WAS"
    "SKNDIAG.WAS" = "SK016.WAS"
    "SKSYSM.WAS" = "SK017.WAS"
    "SKVIRT.WAS" = "SK018.WAS"
    "SKVNET.WAS" = "SK019.WAS"
    "SKVBLK.WAS" = "SK020.WAS"
    "SKVINP.WAS" = "SK021.WAS"
    "SKVGPU.WAS" = "SK022.WAS"
    "SKVAGP.WAS" = "SK023.WAS"
    "SKDSK.WAS" = "SK024.WAS"
}

foreach ($entry in $slotMap.GetEnumerator()) {
    $sourcePath = Join-Path $outDir $entry.Key
    $targetPath = Join-Path $outDir $entry.Value
    Write-FixedSkillSlot -SourcePath $sourcePath -TargetPath $targetPath -Size $fixedSlotSize
    Write-Host ("Seeded fixed slot " + $entry.Value + " from " + $entry.Key)
}
