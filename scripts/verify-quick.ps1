param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path -LiteralPath $Root).Path

function Invoke-Checked([string]$Label, [scriptblock]$Command) {
    Write-Host "==> $Label"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

Push-Location $root
try {
    Invoke-Checked "cargo fmt --check" { cargo fmt --check }
    Invoke-Checked "cargo clippy" {
        cargo clippy --workspace --exclude ade-desktop-app --all-targets -- -D warnings
    }
    Push-Location (Join-Path $root "apps/desktop")
    try {
        Invoke-Checked "desktop TypeScript" { npx tsc --noEmit }
    } finally {
        Pop-Location
    }
} finally {
    Pop-Location
}

Write-Host "verify-quick passed"
