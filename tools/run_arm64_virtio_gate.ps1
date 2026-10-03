param(
    [switch]$SkipBuild,
    [switch]$Quick,
    [switch]$Sustained,
    [switch]$TrafficWindow,
    [switch]$DeviceGates,
    [switch]$LiveCutover,
    [switch]$LiveRouting,
    [switch]$LiveRouteAll,
    [switch]$LiveBootService,
    [switch]$LiveServiceLoop,
    [switch]$OnlyLiveServiceLoop,
    [switch]$LiveOsPump,
    [switch]$OnlyLiveOsPump,
    [switch]$LiveOsPumpLong,
    [switch]$OnlyLiveOsPumpLong,
    [switch]$LiveOsRequestApi,
    [switch]$OnlyLiveOsRequestApi,
    [switch]$LiveOsRuntime,
    [switch]$OnlyLiveOsRuntime,
    [switch]$LiveOsSubsystemBridge,
    [switch]$OnlyLiveOsSubsystemBridge,
    [switch]$LiveSubsystemRuntime,
    [switch]$OnlyLiveSubsystemRuntime,
    [switch]$LiveProductionQueue,
    [switch]$OnlyLiveProductionQueue,
    [switch]$LiveProductionRuntime,
    [switch]$OnlyLiveProductionRuntime,
    [switch]$LiveProductionSubsystemSource,
    [switch]$OnlyLiveProductionSubsystemSource,
    [switch]$LiveProductionSubsystemSourceLive,
    [switch]$OnlyLiveProductionSubsystemSourceLive,
    [switch]$LiveProductionSubsystemSourceLiveSoak,
    [switch]$OnlyLiveProductionSubsystemSourceLiveSoak,
    [switch]$LiveProductionRealSubsystemSourceLive,
    [switch]$OnlyLiveProductionRealSubsystemSourceLive,
    [switch]$LiveProductionRealSubsystemSourceLiveSoak,
    [switch]$OnlyLiveProductionRealSubsystemSourceLiveSoak,
    [switch]$LiveProductionNormalSubsystemSourceLive,
    [switch]$OnlyLiveProductionNormalSubsystemSourceLive,
    [switch]$LiveProductionNormalSubsystemSourceLiveSoak,
    [switch]$OnlyLiveProductionNormalSubsystemSourceLiveSoak,
    [switch]$LiveProductionSandboxSourceLive,
    [switch]$OnlyLiveProductionSandboxSourceLive,
    [switch]$LiveProductionSandboxSourceLiveSoak,
    [switch]$OnlyLiveProductionSandboxSourceLiveSoak,
    [switch]$LiveProductionExternalNormalSourceLive,
    [switch]$OnlyLiveProductionExternalNormalSourceLive,
    [switch]$LiveProductionHardwarePayloadSourceLive,
    [switch]$OnlyLiveProductionHardwarePayloadSourceLive,
    [switch]$LiveProductionHardwarePayloadSourceDeviceLive,
    [switch]$OnlyLiveProductionHardwarePayloadSourceDeviceLive,
    [switch]$LiveProductionHardwarePayloadSourceDeviceLiveSoak,
    [switch]$OnlyLiveProductionHardwarePayloadSourceDeviceLiveSoak,
    [switch]$LiveProductionHardwarePayloadPromoteRollbackDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPromoteRollbackDevice,
    [switch]$LiveProductionHardwarePayloadPolicyCandidateDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPolicyCandidateDevice,
    [switch]$LiveProductionHardwarePayloadPolicyCapsuleDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPolicyCapsuleDevice,
    [switch]$LiveProductionHardwarePayloadPolicyStorageDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPolicyStorageDevice,
    [switch]$LiveProductionHardwarePayloadPolicyStorageProofDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPolicyStorageProofDevice,
    [switch]$LiveProductionHardwarePayloadPolicyStorageProviderProofDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPolicyStorageProviderProofDevice,
    [switch]$LiveProductionHardwarePayloadPolicyStorageProviderRestoreProofDevice,
    [switch]$OnlyLiveProductionHardwarePayloadPolicyStorageProviderRestoreProofDevice,
    [switch]$LiveProductionExternalNormalDeviceSubmitLive,
    [switch]$OnlyLiveProductionExternalNormalDeviceSubmitLive,
    [switch]$LiveProductionExternalNormalDeviceSubmitSoak,
    [switch]$OnlyLiveProductionExternalNormalDeviceSubmitSoak,
    [switch]$LiveProductionSandboxSourceDeviceLive,
    [switch]$OnlyLiveProductionSandboxSourceDeviceLive,
    [switch]$LiveProductionSandboxSourceDeviceLiveSoak,
    [switch]$OnlyLiveProductionSandboxSourceDeviceLiveSoak,
    [switch]$LiveProductionNormalBootLive,
    [switch]$OnlyLiveProductionNormalBootLive,
    [switch]$LiveProductionNormalBootLiveSoak,
    [switch]$OnlyLiveProductionNormalBootLiveSoak,
    [switch]$LiveProductionRealSubsystemSourceDeviceLive,
    [switch]$OnlyLiveProductionRealSubsystemSourceDeviceLive,
    [switch]$LiveProductionRealSubsystemSourceDeviceLiveSoak,
    [switch]$OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak,
    [switch]$ProductionCandidate,
    [switch]$OnlyProductionCandidate,
    [switch]$LiveProductionBootGate,
    [switch]$OnlyLiveProductionBootGate,
    [switch]$LiveProductionBootLive,
    [switch]$OnlyLiveProductionBootLive,
    [switch]$LiveProductionBootLiveSoak,
    [switch]$OnlyLiveProductionBootLiveSoak,
    [switch]$LiveProductionDeviceBootGates,
    [switch]$OnlyLiveProductionDeviceBootGates,
    [switch]$LiveProductionDeviceBootLiveGates,
    [switch]$OnlyLiveProductionDeviceBootLiveGates,
    [switch]$LiveManualSubsystem,
    [switch]$OnlyLiveManualSubsystem,
    [ValidateRange(1, 1024)]
    [int]$ProductionRuntimeBatches = 8
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Action
    )

    Write-Host ""
    Write-Host ("== " + $Name) -ForegroundColor Cyan
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw ("Step failed: " + $Name)
    }
}

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        Invoke-Step "Build sandbox skills" {
            & powershell.exe -ExecutionPolicy Bypass -File .\build_sandbox_skills.ps1
        }
        Invoke-Step "Build ARM64 recovery ELF" {
            & powershell.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1
        }
    }

    if (-not $OnlyLiveServiceLoop -and -not $OnlyLiveOsPump -and -not $OnlyLiveOsPumpLong -and -not $OnlyLiveOsRequestApi -and -not $OnlyLiveOsRuntime -and -not $OnlyLiveOsSubsystemBridge -and -not $OnlyLiveSubsystemRuntime -and -not $OnlyLiveProductionQueue -and -not $OnlyLiveProductionRuntime -and -not $OnlyLiveProductionSubsystemSource -and -not $OnlyLiveProductionSubsystemSourceLive -and -not $OnlyLiveProductionSubsystemSourceLiveSoak -and -not $OnlyLiveProductionRealSubsystemSourceLive -and -not $OnlyLiveProductionRealSubsystemSourceLiveSoak -and -not $OnlyLiveProductionNormalSubsystemSourceLive -and -not $OnlyLiveProductionNormalSubsystemSourceLiveSoak -and -not $OnlyLiveProductionSandboxSourceLive -and -not $OnlyLiveProductionSandboxSourceLiveSoak -and -not $OnlyLiveProductionExternalNormalSourceLive -and -not $OnlyLiveProductionHardwarePayloadSourceLive -and -not $OnlyLiveProductionHardwarePayloadSourceDeviceLive -and -not $OnlyLiveProductionHardwarePayloadSourceDeviceLiveSoak -and -not $OnlyLiveProductionHardwarePayloadPromoteRollbackDevice -and -not $OnlyLiveProductionHardwarePayloadPolicyCandidateDevice -and -not $OnlyLiveProductionHardwarePayloadPolicyCapsuleDevice -and -not $OnlyLiveProductionHardwarePayloadPolicyStorageDevice -and -not $OnlyLiveProductionHardwarePayloadPolicyStorageProofDevice -and -not $OnlyLiveProductionHardwarePayloadPolicyStorageProviderProofDevice -and -not $OnlyLiveProductionHardwarePayloadPolicyStorageProviderRestoreProofDevice -and -not $OnlyLiveProductionExternalNormalDeviceSubmitLive -and -not $OnlyLiveProductionExternalNormalDeviceSubmitSoak -and -not $OnlyLiveProductionSandboxSourceDeviceLive -and -not $OnlyLiveProductionSandboxSourceDeviceLiveSoak -and -not $OnlyLiveProductionNormalBootLive -and -not $OnlyLiveProductionNormalBootLiveSoak -and -not $OnlyLiveProductionRealSubsystemSourceDeviceLive -and -not $OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak -and -not $OnlyProductionCandidate -and -not $OnlyLiveProductionBootGate -and -not $OnlyLiveProductionBootLive -and -not $OnlyLiveProductionBootLiveSoak -and -not $OnlyLiveProductionDeviceBootGates -and -not $OnlyLiveProductionDeviceBootLiveGates -and -not $OnlyLiveManualSubsystem) {
        Invoke-Step "ARM64 tiny-executor rollback gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-rollback --timeout 60
        }

        Invoke-Step "ARM64 full Wasm promotion proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-promotion-gate --timeout 90
        }

        if (-not $Quick) {
            Invoke-Step "ARM64 sustained full Wasm rollback window" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-sustained-gate --timeout 240
            }
            Invoke-Step "ARM64 full Wasm traffic-counter window" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-traffic-window --timeout 360
            }
            Invoke-Step "ARM64 per-device full Wasm block enable gate" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate block --timeout 360
            }
            Invoke-Step "ARM64 per-device full Wasm GPU enable gate" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate gpu --timeout 360
            }
            Invoke-Step "ARM64 per-device full Wasm net enable gate" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate net --timeout 360
            }
            Invoke-Step "ARM64 per-device full Wasm input enable gate" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate input --timeout 360
            }
            Invoke-Step "ARM64 injected input/net RX tiny-executor path" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-injected --timeout 60
            }
            Invoke-Step "ARM64 Rust-generated Wasm call probe" {
                & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-rust-probe --timeout 60
            }
        }
    }
    if ($Quick -and $Sustained) {
        Invoke-Step "ARM64 sustained full Wasm rollback window" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-sustained-gate --timeout 240
        }
    }
    if ($Quick -and $TrafficWindow) {
        Invoke-Step "ARM64 full Wasm traffic-counter window" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-traffic-window --timeout 360
        }
    }
    if ($Quick -and $DeviceGates) {
        Invoke-Step "ARM64 per-device full Wasm block enable gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate block --timeout 360
        }
        Invoke-Step "ARM64 per-device full Wasm GPU enable gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate gpu --timeout 360
        }
        Invoke-Step "ARM64 per-device full Wasm net enable gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate net --timeout 360
        }
        Invoke-Step "ARM64 per-device full Wasm input enable gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-enable-gate input --timeout 360
        }
    }
    if ($LiveCutover) {
        Invoke-Step "ARM64 virtio block live owner cutover rollback" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover block --timeout 360
        }
        Invoke-Step "ARM64 virtio GPU live owner cutover rollback" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover gpu --timeout 360
        }
        Invoke-Step "ARM64 virtio net live owner cutover rollback" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover net --timeout 360
        }
        Invoke-Step "ARM64 virtio input live owner cutover rollback" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-device-live-cutover input --timeout 360
        }
    }
    if ($LiveRouting) {
        Invoke-Step "ARM64 virtio production live routing diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-routing --timeout 720
        }
    }
    if ($LiveRouteAll) {
        Invoke-Step "ARM64 virtio production live route-all diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-route-all --timeout 720
        }
    }
    if ($LiveBootService) {
        Invoke-Step "ARM64 virtio post-boot live service diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-boot-service --timeout 900
        }
    }
    if ($LiveServiceLoop -or $OnlyLiveServiceLoop) {
        Invoke-Step "ARM64 virtio mainloop live service diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-service-loop --timeout 900
        }
    }
    if ($LiveOsPump -or $OnlyLiveOsPump) {
        Invoke-Step "ARM64 virtio mainloop typed OS request/event pump diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-pump --timeout 900
        }
    }
    if ($LiveOsPumpLong -or $OnlyLiveOsPumpLong) {
        Invoke-Step "ARM64 virtio long-window typed OS request/event pump diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-pump-long --timeout 1200
        }
    }
    if ($LiveOsRequestApi -or $OnlyLiveOsRequestApi) {
        Invoke-Step "ARM64 virtio OS producer API request/event pump diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-request-api --timeout 900
        }
    }
    if ($LiveOsRuntime -or $OnlyLiveOsRuntime) {
        Invoke-Step "ARM64 virtio opt-in live OS runtime producer diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-runtime --timeout 900
        }
    }
    if ($LiveOsSubsystemBridge -or $OnlyLiveOsSubsystemBridge) {
        Invoke-Step "ARM64 virtio subsystem bridge producer diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-os-subsystem-bridge --timeout 1200
        }
    }
    if ($LiveSubsystemRuntime -or $OnlyLiveSubsystemRuntime) {
        Invoke-Step "ARM64 virtio persistent subsystem runtime diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-subsystem-runtime --timeout 1800
        }
    }
    if ($LiveProductionQueue -or $OnlyLiveProductionQueue) {
        Invoke-Step "ARM64 virtio production subsystem inbox queue diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-queue --timeout 1200
        }
    }
    if ($LiveProductionRuntime -or $OnlyLiveProductionRuntime) {
        Invoke-Step "ARM64 virtio opt-in production inbox runtime diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-runtime --production-runtime-batches $ProductionRuntimeBatches --timeout 1200
        }
    }
    if ($LiveProductionSubsystemSource -or $OnlyLiveProductionSubsystemSource) {
        Invoke-Step "ARM64 virtio production subsystem source runtime" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-subsystem-source --timeout 1800
        }
    }
    if ($LiveProductionSubsystemSourceLive -or $OnlyLiveProductionSubsystemSourceLive) {
        Invoke-Step "ARM64 virtio production subsystem source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-subsystem-source-live --timeout 1800
        }
    }
    if ($LiveProductionSubsystemSourceLiveSoak -or $OnlyLiveProductionSubsystemSourceLiveSoak) {
        Invoke-Step "ARM64 virtio production subsystem source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-subsystem-source-live-soak --timeout 3600
        }
    }
    if ($LiveProductionRealSubsystemSourceLive -or $OnlyLiveProductionRealSubsystemSourceLive) {
        Invoke-Step "ARM64 virtio production real-device source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live --timeout 1800
        }
    }
    if ($LiveProductionRealSubsystemSourceLiveSoak -or $OnlyLiveProductionRealSubsystemSourceLiveSoak) {
        Invoke-Step "ARM64 virtio production real-device source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-soak --timeout 3600
        }
    }
    if ($LiveProductionNormalSubsystemSourceLive -or $OnlyLiveProductionNormalSubsystemSourceLive) {
        Invoke-Step "ARM64 virtio normal-subsystem producer source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-normal-subsystem-source-live --timeout 1800
        }
    }
    if ($LiveProductionNormalSubsystemSourceLiveSoak -or $OnlyLiveProductionNormalSubsystemSourceLiveSoak) {
        Invoke-Step "ARM64 virtio normal-subsystem producer source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-normal-subsystem-source-live-soak --timeout 3600
        }
    }
    if ($LiveProductionSandboxSourceLive -or $OnlyLiveProductionSandboxSourceLive) {
        Invoke-Step "ARM64 virtio sandbox-driver source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live --timeout 1800
        }
    }
    if ($LiveProductionSandboxSourceLiveSoak -or $OnlyLiveProductionSandboxSourceLiveSoak) {
        Invoke-Step "ARM64 virtio sandbox-driver source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-soak --timeout 3600
        }
    }
    if ($LiveProductionExternalNormalSourceLive -or $OnlyLiveProductionExternalNormalSourceLive) {
        Invoke-Step "ARM64 virtio external normal subsystem source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-external-normal-source-live --timeout 1800
        }
    }
    if ($LiveProductionHardwarePayloadSourceLive -or $OnlyLiveProductionHardwarePayloadSourceLive) {
        Invoke-Step "ARM64 virtio hardware payload source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-hardware-payload-source-live --timeout 1800
        }
    }
    if ($LiveProductionHardwarePayloadSourceDeviceLive -or $OnlyLiveProductionHardwarePayloadSourceDeviceLive) {
        Invoke-Step "ARM64 virtio block hardware payload source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device block --timeout 1800
        }
        Invoke-Step "ARM64 virtio GPU hardware payload source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device gpu --timeout 1800
        }
        Invoke-Step "ARM64 virtio net hardware payload source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device net --timeout 1800
        }
        Invoke-Step "ARM64 virtio input hardware payload source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device input --timeout 1800
        }
    }
    if ($LiveProductionHardwarePayloadSourceDeviceLiveSoak -or $OnlyLiveProductionHardwarePayloadSourceDeviceLiveSoak) {
        Invoke-Step "ARM64 virtio block hardware payload source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device-soak block --timeout 3600
        }
        Invoke-Step "ARM64 virtio GPU hardware payload source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device-soak gpu --timeout 3600
        }
        Invoke-Step "ARM64 virtio net hardware payload source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device-soak net --timeout 3600
        }
        Invoke-Step "ARM64 virtio input hardware payload source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-source-live-device-soak input --timeout 3600
        }
    }
    if ($LiveProductionHardwarePayloadPromoteRollbackDevice -or $OnlyLiveProductionHardwarePayloadPromoteRollbackDevice) {
        Invoke-Step "ARM64 virtio block hardware payload promote-dispatch-rollback gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-promote-rollback-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload promote-dispatch-rollback gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-promote-rollback-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload promote-dispatch-rollback gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-promote-rollback-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload promote-dispatch-rollback gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-promote-rollback-device input --timeout 240
        }
    }
    if ($LiveProductionHardwarePayloadPolicyCandidateDevice -or $OnlyLiveProductionHardwarePayloadPolicyCandidateDevice) {
        Invoke-Step "ARM64 virtio block hardware payload boot-policy candidate gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-candidate-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload boot-policy candidate gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-candidate-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload boot-policy candidate gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-candidate-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload boot-policy candidate gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-candidate-device input --timeout 240
        }
    }
    if ($LiveProductionHardwarePayloadPolicyCapsuleDevice -or $OnlyLiveProductionHardwarePayloadPolicyCapsuleDevice) {
        Invoke-Step "ARM64 virtio block hardware payload boot-policy capsule gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-capsule-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload boot-policy capsule gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-capsule-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload boot-policy capsule gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-capsule-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload boot-policy capsule gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-capsule-device input --timeout 240
        }
    }
    if ($LiveProductionHardwarePayloadPolicyStorageDevice -or $OnlyLiveProductionHardwarePayloadPolicyStorageDevice) {
        Invoke-Step "ARM64 virtio block hardware payload policy storage ABI gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload policy storage ABI gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload policy storage ABI gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload policy storage ABI gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-device input --timeout 240
        }
    }
    if ($LiveProductionHardwarePayloadPolicyStorageProofDevice -or $OnlyLiveProductionHardwarePayloadPolicyStorageProofDevice) {
        Invoke-Step "ARM64 virtio block hardware payload policy storage proof-capability gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-proof-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload policy storage proof-capability gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-proof-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload policy storage proof-capability gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-proof-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload policy storage proof-capability gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-proof-device input --timeout 240
        }
    }
    if ($LiveProductionHardwarePayloadPolicyStorageProviderProofDevice -or $OnlyLiveProductionHardwarePayloadPolicyStorageProviderProofDevice) {
        Invoke-Step "ARM64 virtio block hardware payload policy storage provider proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-proof-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload policy storage provider proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-proof-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload policy storage provider proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-proof-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload policy storage provider proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-proof-device input --timeout 240
        }
    }
    if ($LiveProductionHardwarePayloadPolicyStorageProviderRestoreProofDevice -or $OnlyLiveProductionHardwarePayloadPolicyStorageProviderRestoreProofDevice) {
        Invoke-Step "ARM64 virtio block hardware payload policy storage provider restore proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-restore-proof-device block --timeout 240
        }
        Invoke-Step "ARM64 virtio GPU hardware payload policy storage provider restore proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-restore-proof-device gpu --timeout 240
        }
        Invoke-Step "ARM64 virtio net hardware payload policy storage provider restore proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-restore-proof-device net --timeout 240
        }
        Invoke-Step "ARM64 virtio input hardware payload policy storage provider restore proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-hardware-payload-policy-storage-provider-restore-proof-device input --timeout 240
        }
    }
    if ($LiveProductionExternalNormalDeviceSubmitLive -or $OnlyLiveProductionExternalNormalDeviceSubmitLive) {
        Invoke-Step "ARM64 virtio external normal subsystem per-device submit proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-external-normal-device-submit-live --timeout 1800
        }
    }
    if ($LiveProductionExternalNormalDeviceSubmitSoak -or $OnlyLiveProductionExternalNormalDeviceSubmitSoak) {
        Invoke-Step "ARM64 virtio external normal subsystem repeated device-round submit proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-external-normal-device-submit-soak --external-normal-device-submit-batches 8 --timeout 1800
        }
    }
    if ($LiveProductionSandboxSourceDeviceLive -or $OnlyLiveProductionSandboxSourceDeviceLive) {
        Invoke-Step "ARM64 virtio block sandbox-driver source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device block --timeout 1800
        }
        Invoke-Step "ARM64 virtio GPU sandbox-driver source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device gpu --timeout 1800
        }
        Invoke-Step "ARM64 virtio net sandbox-driver source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device net --timeout 1800
        }
        Invoke-Step "ARM64 virtio input sandbox-driver source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device input --timeout 1800
        }
    }
    if ($LiveProductionSandboxSourceDeviceLiveSoak -or $OnlyLiveProductionSandboxSourceDeviceLiveSoak) {
        Invoke-Step "ARM64 virtio block sandbox-driver source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak block --timeout 3600
        }
        Invoke-Step "ARM64 virtio GPU sandbox-driver source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak gpu --timeout 3600
        }
        Invoke-Step "ARM64 virtio net sandbox-driver source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak net --timeout 3600
        }
        Invoke-Step "ARM64 virtio input sandbox-driver source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak input --timeout 3600
        }
    }
    if ($LiveProductionNormalBootLive -or $OnlyLiveProductionNormalBootLive) {
        Invoke-Step "ARM64 virtio normal-subsystem producer boot-live gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-normal-boot-live --timeout 1800
        }
    }
    if ($LiveProductionNormalBootLiveSoak -or $OnlyLiveProductionNormalBootLiveSoak) {
        Invoke-Step "ARM64 virtio normal-subsystem producer boot-live long soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-normal-boot-live-soak --timeout 3600
        }
    }
    if ($LiveProductionRealSubsystemSourceDeviceLive -or $OnlyLiveProductionRealSubsystemSourceDeviceLive) {
        Invoke-Step "ARM64 virtio block production real-device source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device block --timeout 1800
        }
        Invoke-Step "ARM64 virtio GPU production real-device source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device gpu --timeout 1800
        }
        Invoke-Step "ARM64 virtio net production real-device source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device net --timeout 1800
        }
        Invoke-Step "ARM64 virtio input production real-device source-live routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device input --timeout 1800
        }
    }
    if ($LiveProductionRealSubsystemSourceDeviceLiveSoak -or $OnlyLiveProductionRealSubsystemSourceDeviceLiveSoak) {
        Invoke-Step "ARM64 virtio block production real-device source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak block --timeout 3600
        }
        Invoke-Step "ARM64 virtio GPU production real-device source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak gpu --timeout 3600
        }
        Invoke-Step "ARM64 virtio net production real-device source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak net --timeout 3600
        }
        Invoke-Step "ARM64 virtio input production real-device source-live long soak routing gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak input --timeout 3600
        }
    }
    if ($ProductionCandidate -or $OnlyProductionCandidate) {
        Invoke-Step "ARM64 production candidate: all detected-device source-live gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live --timeout 1800
        }
        Invoke-Step "ARM64 production candidate: all detected-device source-live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-soak --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: normal-subsystem producer source-live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-normal-subsystem-source-live-soak --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: sandbox-driver source live proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live --timeout 1800
        }
        Invoke-Step "ARM64 production candidate: external normal-subsystem submit proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-external-normal-source-live --timeout 1800
        }
        Invoke-Step "ARM64 production candidate: external normal-subsystem per-device submit proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-external-normal-device-submit-live --timeout 1800
        }
        Invoke-Step "ARM64 production candidate: external normal-subsystem repeated device-round submit proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-external-normal-device-submit-soak --external-normal-device-submit-batches 8 --timeout 1800
        }
        Invoke-Step "ARM64 production candidate: sandbox-driver source live soak proof gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-soak --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: block sandbox-driver source live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak block --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: GPU sandbox-driver source live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak gpu --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: net sandbox-driver source live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak net --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: input sandbox-driver source live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-sandbox-source-live-device-soak input --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: normal-subsystem producer boot-live gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-normal-boot-live --timeout 1800
        }
        Invoke-Step "ARM64 production candidate: block detected-device source-live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak block --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: GPU detected-device source-live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak gpu --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: net detected-device source-live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak net --timeout 3600
        }
        Invoke-Step "ARM64 production candidate: input detected-device source-live soak gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --inject-input --inject-net-rx --virtio-wasm-live-production-real-subsystem-source-live-device-soak input --timeout 3600
        }
    }
    if ($LiveProductionBootGate -or $OnlyLiveProductionBootGate) {
        Invoke-Step "ARM64 virtio opt-in production boot promotion gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-boot-gate --timeout 1800
        }
    }
    if ($LiveProductionBootLive -or $OnlyLiveProductionBootLive) {
        Invoke-Step "ARM64 virtio opt-in production boot-live keepalive gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-boot-live --timeout 1800
        }
    }
    if ($LiveProductionBootLiveSoak -or $OnlyLiveProductionBootLiveSoak) {
        Invoke-Step "ARM64 virtio opt-in production boot-live long soak keepalive gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-boot-live-soak --timeout 3600
        }
    }
    if ($LiveProductionDeviceBootGates -or $OnlyLiveProductionDeviceBootGates) {
        Invoke-Step "ARM64 virtio block production boot promotion gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-gate block --timeout 1800
        }
        Invoke-Step "ARM64 virtio GPU production boot promotion gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-gate gpu --timeout 1800
        }
        Invoke-Step "ARM64 virtio net production boot promotion gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-gate net --timeout 1800
        }
        Invoke-Step "ARM64 virtio input production boot promotion gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-gate input --timeout 1800
        }
    }
    if ($LiveProductionDeviceBootLiveGates -or $OnlyLiveProductionDeviceBootLiveGates) {
        Invoke-Step "ARM64 virtio block production boot-live keepalive gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-live block --timeout 1800
        }
        Invoke-Step "ARM64 virtio GPU production boot-live keepalive gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-live gpu --timeout 1800
        }
        Invoke-Step "ARM64 virtio net production boot-live keepalive gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-live net --timeout 1800
        }
        Invoke-Step "ARM64 virtio input production boot-live keepalive gate" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-production-device-boot-live input --timeout 1800
        }
    }
    if ($LiveManualSubsystem -or $OnlyLiveManualSubsystem) {
        Invoke-Step "ARM64 virtio manual subsystem submit diagnostic" {
            & python .\tools\smoke_arm64_recovery.py --virtio-payload --virtio-wasm-live-manual-subsystem --timeout 1200
        }
    }

    Write-Host ""
    Write-Host "ARM64 virtio sandbox gates passed. Sustained, traffic-window, per-device evidence, explicit live cutover rollback, live routing, route-all, post-boot live service, mainloop service, short/long typed OS pump, producer API, opt-in live runtime, subsystem bridge, persistent subsystem runtime, production inbox queue/runtime, subsystem source-live/long-soak gates, detected-device source-live/long-soak gates, normal-subsystem producer source-live/long-soak gates, sandbox-driver source proof/soak gates, external normal-subsystem submit proof gate, device-scoped sandbox-source gates, device-scoped detected-source gates, production-candidate gate bundle, boot promotion gates, boot-live keepalive/long-soak gates, device boot promotion/boot-live gates, and manual subsystem submit diagnostics are available when those steps run; boot autorun remains disabled unless a persisted policy enables it." -ForegroundColor Green
} finally {
    Pop-Location
}
