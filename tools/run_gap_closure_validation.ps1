param(
    [switch]$Quick,
    [switch]$RunArm64Quick,
    [switch]$RunArm64ProductionCandidate,
    [string]$SemanticRoot = "filesystem-skills",
    [string]$SemanticQuery = "filesystem validation read write restore remount"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RepoRoot

$PythonCandidates = @(
    "D:\Codex\.venv\Scripts\python.exe",
    "D:\python\anaconda3\python.exe",
    "python"
)
$Python = $null
foreach ($Candidate in $PythonCandidates) {
    $Resolved = Get-Command $Candidate -ErrorAction SilentlyContinue
    if ($Resolved) {
        $Python = $Resolved.Source
        break
    }
}
if (-not $Python) {
    throw "No usable Python interpreter found"
}

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Script
    )
    Write-Host "[gap-validation] $Name"
    & $Script
}

Invoke-Step "platform matrix" {
    & $Python .\tools\platform_matrix.py --strict
}

if (-not $Quick) {
    Invoke-Step "filesystem family validation matrix" {
        & $Python .\tools\storage_fs_skill_lab.py
    }
} else {
    Write-Host "[gap-validation] filesystem family validation matrix skipped by -Quick"
}

Invoke-Step "semantic graph index, validate, and query" {
    & $Python .\tools\semantic_graph_layer.py `
        --root $SemanticRoot `
        --output .\logs\semantic-graph-gap-validation `
        --validate `
        --query $SemanticQuery `
        --limit 8
}

Invoke-Step "GUI public-alpha evidence report" {
    & $Python .\tools\gui_public_alpha_regression.py --allow-missing-runtime
}

if ($RunArm64Quick) {
    Invoke-Step "ARM64 virtio quick gate" {
        powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -Quick
    }
}

if ($RunArm64ProductionCandidate) {
    Invoke-Step "ARM64 production candidate proof gate" {
        powershell.exe -ExecutionPolicy Bypass -File .\tools\run_arm64_virtio_gate.ps1 -SkipBuild -OnlyProductionCandidate
    }
}

Write-Host "[gap-validation] complete"
