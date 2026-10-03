param()

$ErrorActionPreference = "Stop"
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$testParent = [IO.Path]::GetFullPath((Join-Path $repoRoot ".baseline-tests"))
$fixture = Join-Path $testParent ([guid]::NewGuid().ToString("N"))
$verify = Join-Path $fixture "next/tools/verify-baseline.ps1"
$utf8 = New-Object System.Text.UTF8Encoding($false)
$passed = 0

function Expect-Rejection([string]$Name, [string]$RelativePath, [scriptblock]$Change, [string]$Expected) {
    $path = Join-Path $fixture $RelativePath
    $bytes = [IO.File]::ReadAllBytes($path)
    try {
        & $Change $path
        $rejected = $false
        try { & $verify | Out-Null } catch {
            if ($_.Exception.Message -notmatch $Expected) { throw }
            $rejected = $true
        }
        if (-not $rejected) { throw "Negative control was accepted: $Name" }
        Write-Output "PASS: rejects $Name"
        $script:passed++
    } finally {
        [IO.File]::WriteAllBytes($path, $bytes)
    }
}

try {
    New-Item -ItemType Directory -Path $fixture -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot "next") -Destination (Join-Path $fixture "next") -Recurse
    foreach ($name in @("README.md", "README.en.md", "ARCHIVE.md", "CONTRIBUTING.md", "SECURITY.md", "AGENTS.md", "LICENSE")) {
        Copy-Item -LiteralPath (Join-Path $repoRoot $name) -Destination (Join-Path $fixture $name)
    }
    & $verify | Out-Null
    $passed++
    Write-Output "PASS: intact baseline"

    Expect-Rejection "missing Korean review" "next/docs/ASTRA_PLAN_REVIEW.ko.md" {
        param($path)
        Remove-Item -LiteralPath $path -Force
    } "Missing required document"

    Expect-Rejection "translation identifier drift" "next/docs/REQUIREMENTS.ko.md" {
        param($path)
        $text = [IO.File]::ReadAllText($path).Replace("R25", "R26")
        [IO.File]::WriteAllText($path, $text, $utf8)
    } "Translation identifier/source mismatch"

    Expect-Rejection "broken local link" "README.md" {
        param($path)
        [IO.File]::AppendAllText($path, "[broken](missing-document.md)", $utf8)
    } "Broken local link"

    Expect-Rejection "changed historical inventory" "next/docs/legacy-inventory.json" {
        param($path)
        [IO.File]::AppendAllText($path, " ", $utf8)
    } "Frozen inventory hash mismatch"

    & $verify | Out-Null
    $passed++
    Write-Output "PASS: restored baseline"
    Write-Output "Baseline verifier tests passed: $passed cases. No runtime testing is implied."
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    if (-not $resolved.StartsWith($testParent + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing cleanup outside the test directory."
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
