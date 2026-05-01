param(
    [string]$RegistryBase = "https://openrhiza.com",
    [string]$NodeId = "codex-openrhiza-maintainer",
    [switch]$DryRun,
    [switch]$ContinueOnError
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$driverDisk = Join-Path $repoRoot "rhiza_drivers"

function Convert-FileToHex {
    param([Parameter(Mandatory = $true)][string]$Path)

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    return [System.BitConverter]::ToString($bytes).Replace("-", "").ToLowerInvariant()
}

function Convert-FileToBase64Text {
    param([Parameter(Mandatory = $true)][string]$Path)

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    return "base64:" + [System.Convert]::ToBase64String($bytes)
}

function Invoke-RegistryPost {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][hashtable]$Body
    )

    $uri = "$RegistryBase$Path"
    $json = $Body | ConvertTo-Json -Depth 12 -Compress

    if ($DryRun) {
        Write-Host "[dry-run] POST $uri"
        return $null
    }

    try {
        $response = Invoke-RestMethod -Method Post -Uri $uri -ContentType "application/json" -Body $json
        Write-Host "[ok] $Path"
        return $response
    } catch {
        Write-Host "[failed] $Path :: $($_.Exception.Message)" -ForegroundColor Red
        if (-not $ContinueOnError) {
            throw
        }
        return $null
    }
}

function ArtifactPath {
    param([Parameter(Mandatory = $true)][string]$Name)
    return Join-Path $driverDisk $Name
}

Write-Host "OpenRhiza registry sync target: $RegistryBase"
Write-Host "Node: $NodeId"

Invoke-RegistryPost "/api/v1/node/register" @{
    protocol_version = "v1"
    node_id = $NodeId
    public_key = "codex-maintainer-software-key"
    identity_type = "software_key"
    tpm_present = $false
    os_version = "openrhiza-dev-main"
    transport_capabilities = @("tls", "http_json", "signed_wasm", "driver_download")
} | Out-Null

$skills = @(
    @{ id = "skill_registry_lookup_v1"; name = "Registry Lookup"; category = "registry"; file = "SKREG.WAS"; summary = "Searches OpenRhiza capability records before generation or activation."; rec = @("driver lookup", "program lookup", "skill lookup") },
    @{ id = "skill_display_console_mode_v1"; name = "Display Console Mode"; category = "display"; file = "SKDSP.WAS"; summary = "Requests a sandbox-owned 1920x1080 wide-console session."; rec = @("1920x1080 console", "wide console", "display transition") },
    @{ id = "skill_gui_session_bootstrap_v1"; name = "GUI Session Bootstrap"; category = "display"; file = "SKGUI.WAS"; summary = "Coordinates a sandbox-owned 1920x1080 GUI handoff with rollback."; rec = @("gui bootstrap", "1920x1080 gui", "display orchestration") },
    @{ id = "skill_display_framebuffer_mode_v1"; name = "Display Framebuffer Mode"; category = "display"; file = "SKFBUF.WAS"; summary = "Negotiates a framebuffer-backed console session through the display ABI."; rec = @("1920x1080 framebuffer", "wide console", "display validation") },
    @{ id = "skill_gui_compositor_seed_v1"; name = "GUI Compositor Seed"; category = "display"; file = "SKCOMP.WAS"; summary = "Bootstraps a sandbox GUI compositor session with recovery-shell rollback."; rec = @("gui compositor", "display session", "rollback") },
    @{ id = "skill_gui_scene_mutator_v1"; name = "GUI Scene Mutator"; category = "display"; file = "SKMUT.WAS"; summary = "Applies object-scoped GUI scene mutations without touching unrelated objects."; rec = @("gui mutation", "object scene", "layout refinement") },
    @{ id = "skill_fs_image_probe_v1"; name = "Filesystem Image Probe"; category = "storage"; file = "SKFSP.WAS"; summary = "Probes image-backed filesystem targets through the storage host ABI."; rec = @("filesystem probe", "storage harness", "safe scratch validation") },
    @{ id = "skill_gui_modern_shell_v1"; name = "GUI Modern Shell"; category = "display"; file = "SKMSH.WAS"; summary = "Applies the current modern OpenRhiza shell as sandbox-owned GUI mutations."; rec = @("modern gui", "codex-like shell", "object gui") },
    @{ id = "skill_qemu_driver_pack_v1"; name = "QEMU Driver Pack"; category = "driver"; file = "SKQDRV.WAS"; summary = "Declares QEMU baseline driver bindings and smoke-tests the driver host ABI."; rec = @("qemu drivers", "driver host abi", "bootstrap hardware") },
    @{ id = "skill_voice_capture_bridge_v1"; name = "Voice Capture Bridge"; category = "voice"; file = "SKVOICE.WAS"; summary = "Validates the sandbox voice input chain before real audio capture is wired in."; rec = @("voice input", "speech prompt", "microphone bridge", "hands free") }
)

foreach ($skill in $skills) {
    $path = ArtifactPath $skill.file
    if (-not (Test-Path -LiteralPath $path)) {
        Write-Host "[skip] missing $($skill.file)" -ForegroundColor Yellow
        continue
    }

    Invoke-RegistryPost "/api/v1/skill/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        skill_id = $skill.id
        display_name = $skill.name
        category = $skill.category
        delivery = "sandbox_skill"
        summary = $skill.summary
        recommended_for = $skill.rec
        status = "testing"
        artifact_id = "artifact_$($skill.id)_seed"
        source_type = "seed_local_wasm"
        payload_hex = Convert-FileToHex $path
    } | Out-Null

    Invoke-RegistryPost "/api/v1/evaluation/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        subject_type = "skill"
        subject_id = $skill.id
        subject_label = $skill.name
        stability_score = 80
        performance_score = 75
        notes = @("Seed skill uploaded from the OpenRhiza repository. Treat as testing until validated inside the OS runtime.")
    } | Out-Null
}

$drivers = @(
    @{ id = "drv_e1000_native_v1"; key = "pci:8086:100e"; name = "Intel e1000 Native Bootstrap Driver"; hardware = "Intel 82540EM / QEMU e1000"; file = "e1000.bin"; source = "seed_local_binary" },
    @{ id = "snd_input_keyboard_bootstrap_v1"; key = "acpi:PNP0303"; name = "Sandbox Keyboard Bootstrap Driver"; hardware = "PS/2 keyboard bootstrap input"; file = "KEYBDRV.WAS"; source = "seed_local_wasm" },
    @{ id = "snd_input_mouse_bootstrap_v1"; key = "usb:class:03:01:02"; name = "Sandbox Mouse Bootstrap Driver"; hardware = "USB HID mouse bootstrap input"; file = "MOUSEDRV.WAS"; source = "seed_local_wasm" }
)

foreach ($driver in $drivers) {
    $path = ArtifactPath $driver.file
    if (-not (Test-Path -LiteralPath $path)) {
        Write-Host "[skip] missing $($driver.file)" -ForegroundColor Yellow
        continue
    }

    $upload = Invoke-RegistryPost "/api/v1/driver/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        driver_id = $driver.id
        match_key = $driver.key
        display_name = $driver.name
        hardware = $driver.hardware
        source_type = $driver.source
        model = "codex_seed_upload"
        prompt_hash = "repo_seed_$($driver.file)"
        payload_text = Convert-FileToBase64Text $path
    }

    $driverId = if ($upload -and $upload.success) { $upload.data.driver_id } else { $driver.id }
    Invoke-RegistryPost "/api/v1/evaluation/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        subject_type = "driver"
        subject_id = $driverId
        subject_label = $driver.name
        driver_id = $driverId
        hardware_match_key = $driver.key
        stability_score = 78
        performance_score = 72
        notes = @("Seed driver artifact uploaded from the OpenRhiza repository. Keep native/bootstrap fallback until sandbox runtime validation passes.")
    } | Out-Null
}

$workflows = @(
    @{ id = "workflow_driver_acquire_v1"; name = "Driver Acquire And Promote"; summary = "Find, validate, activate, persist, and report a driver."; steps = @("Inspect local runtime bindings", "Query OpenRhiza.com registry", "Generate if missing", "Sandbox smoke test", "Activate live binding", "Persist preferred binding", "Upload evaluation/comment/vote") },
    @{ id = "workflow_skill_load_v1"; name = "Skill Load And Execute"; summary = "Find, load, execute, and evaluate a sandbox skill."; steps = @("Search local and remote skills", "Load sandbox-safe skill", "Run skill for current task", "Record outcome") },
    @{ id = "workflow_gui_scene_mutate_v1"; name = "GUI Scene Mutate And Validate"; summary = "Apply object-scoped GUI mutations safely."; steps = @("Inspect current GUI scene", "Acquire GUI mutation skill", "Apply object-scoped changes", "Validate redraw and rollback boundaries") },
    @{ id = "workflow_core_boundary_migration_v1"; name = "Core Boundary Migration"; summary = "Move bootstrap fallback logic into sandbox-owned capabilities without breaking recovery paths."; steps = @("Classify module boundary", "Create or fetch sandbox replacement", "Run side-by-side with native fallback", "Promote only after validation", "Keep rollback path", "Record evaluation") },
    @{ id = "workflow_fs_bridge_validate_v1"; name = "Filesystem Bridge Validate"; summary = "Validate filesystem family skills through image-backed storage host ABI."; steps = @("Open harness image", "Probe filesystem", "Read metadata", "Scratch write/read/restore", "Flush", "Record evaluation") },
    @{ id = "workflow_autonomy_council_v1"; name = "Autonomy Council Proposal"; summary = "Run bounded multi-agent autonomy and present reversible proposals."; steps = @("Infer user goal", "Gather bounded evidence", "Run council roles", "Summarize disagreement", "Require approval for risky actions") },
    @{ id = "workflow_voice_prompt_v1"; name = "Voice Prompt With Confirmation"; summary = "Capture a bounded voice clip, transcribe it, show the transcript, then submit only after confirmation."; steps = @("Check voice mode", "Capture bounded audio clip", "Run VAD/transcription skill", "Display editable transcript", "Submit through normal prompt path after confirmation", "Record evaluation") }
)

foreach ($workflow in $workflows) {
    Invoke-RegistryPost "/api/v1/workflow/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        workflow_id = $workflow.id
        display_name = $workflow.name
        summary = $workflow.summary
        status = "testing"
        steps = $workflow.steps
    } | Out-Null
}

$policies = @(
    @{ id = "policy_core_minimalism_v1"; scope = "runtime"; summary = "Keep only mandatory survival paths in core."; rules = @("core owns boot recovery sandbox ABI rollback gates", "device policy belongs in drivers or workflows", "GUI behavior belongs in skills when possible") },
    @{ id = "policy_sandbox_first_capabilities_v1"; scope = "runtime"; summary = "All non-core capabilities should be sandbox-owned by default."; rules = @("query registry before generation", "sandbox before activation", "persist only after validation") },
    @{ id = "policy_object_capability_isolation_v1"; scope = "runtime"; summary = "Capabilities must behave as isolated objects."; rules = @("explicit identity", "explicit lifecycle", "object-scoped mutation", "rollback target required") },
    @{ id = "policy_registry_first_v1"; scope = "workflow"; summary = "Always query registry before generating new capabilities."; rules = @("local cache first", "registry second", "generation third") },
    @{ id = "policy_storage_safe_write_v1"; scope = "storage"; summary = "Storage writes require staged validation."; rules = @("read-only first", "scratch write before real write", "flush and verify", "rollback target required") },
    @{ id = "policy_voice_privacy_v1"; scope = "voice"; summary = "Voice capture is user-controlled, bounded, visible, and transcript-confirmed before action."; rules = @("default off", "autonomy cannot enable microphone", "visible recording state required", "transcript before action", "bounded audio clips only") }
)

foreach ($policy in $policies) {
    Invoke-RegistryPost "/api/v1/policy/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        policy_id = $policy.id
        scope = $policy.scope
        summary = $policy.summary
        status = "active"
        rules = $policy.rules
    } | Out-Null
}

$software = @(
    @{ id = "pkg_terminal_tools_v1"; name = "Terminal Starter Tools"; category = "system"; delivery = "text_bundle"; summary = "Basic CLI-first package set for networked OpenRhiza systems." },
    @{ id = "pkg_diag_console_v1"; name = "Diagnostic Console"; category = "debugging"; delivery = "text_bundle"; summary = "Operator-oriented inspection tools for hardware inventory and service API state." },
    @{ id = "pkg_driver_lab_v1"; name = "Driver Lab"; category = "development"; delivery = "sandbox_package"; summary = "Utilities for generating, validating, and promoting driver candidates from the sandbox." },
    @{ id = "pkg_font_lab_v1"; name = "Font Lab"; category = "display"; delivery = "host_tool_bundle"; summary = "Tools for importing fonts and generating validated OpenRhiza atlas assets." },
    @{ id = "pkg_openrhiza_docs_v1"; name = "OpenRhiza Operating Docs"; category = "documentation"; delivery = "markdown_bundle"; summary = "Authoritative operating rules, boundary audits, API docs, and roadmap documents for OpenRhiza agents." },
    @{ id = "pkg_voice_input_lab_v1"; name = "Voice Input Lab"; category = "voice"; delivery = "sandbox_package"; summary = "Development package for bounded microphone capture, transcription, and prompt confirmation experiments." }
)

foreach ($package in $software) {
    Invoke-RegistryPost "/api/v1/software/upload" @{
        protocol_version = "v1"
        node_id = $NodeId
        package_id = $package.id
        display_name = $package.name
        category = $package.category
        delivery = $package.delivery
        summary = $package.summary
        status = "testing"
    } | Out-Null
}

Write-Host "OpenRhiza registry sync completed."
