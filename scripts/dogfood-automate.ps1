# N3 Dogfood: Automate + owned .ade/dogfood + verify G3
# Usage: pwsh -File scripts/dogfood-automate.ps1
# Optional: -Provider opencode -Model deepseek-v4-flash-free
param(
  [string]$Provider = "opencode",
  [string]$BaseUrl = "https://opencode.ai/zen/v1",
  [string]$Model = "deepseek-v4-flash-free",
  [string]$OwnedPath = ".ade/dogfood"
)

$ErrorActionPreference = "Continue"
$root = "C:\Dev\ade"
Set-Location $root

$ade = if (Test-Path "C:\Dev\ade-target\debug\ade.exe") {
  "C:\Dev\ade-target\debug\ade.exe"
} else {
  "ade"
}

Write-Host "=== N3 Dogfood Automate ==="
Write-Host "Root: $root"
Write-Host "ADE:  $ade"
Write-Host "Prov: $Provider / $Model"
Write-Host "Scope: $OwnedPath"
Write-Host ""

if (-not (Test-Path (Join-Path $root "AGENTS.md"))) {
  Write-Host "FAIL missing AGENTS.md — attach ADE repo first"
  exit 1
}

$locks = @()
Get-Process ade-desktop-app, ade -ErrorAction SilentlyContinue | ForEach-Object {
  $locks += "$($_.ProcessName) pid=$($_.Id)"
}
if ($locks.Count -gt 0) {
  Write-Host "WARN rebuild lock present:"
  $locks | ForEach-Object { Write-Host "  $_" }
  Write-Host "  (ok for this evidence-only Automate; quit before cargo build)"
  Write-Host ""
}

New-Item -ItemType Directory -Force -Path (Join-Path $root $OwnedPath) | Out-Null

$prompt = @"
N3 dogfood Automate acceptance.

Write or update ONLY files under $OwnedPath/.
Create/update .ade/dogfood/automate-acceptance.md using the fs write_file tool with:
- ISO date/time
- autonomy=automate
- owned path=.ade/dogfood
- note that verify-on-complete G3 was requested
- one sentence: ADE dogfood Automate evidence

Use tool fs__write_file (path + content). Do not only activate skills.
Do not edit crates/, apps/, docs/platform/.
Do not rebuild binaries.
"@

Write-Host "-- agent turn (automate + approve owned paths + G3) --"
$env:RUST_LOG = "error"
$out = & $ade agent `
  --provider $Provider `
  --base-url $BaseUrl `
  --model $Model `
  --autonomy automate `
  --approve-owned-paths `
  --owned-path $OwnedPath `
  --verify-on-complete `
  --verify-gate G3 `
  --max-steps 6 `
  --input-cost-per-mtok 0 `
  --output-cost-per-mtok 0 `
  $prompt 2>&1 | Out-String

$code = $LASTEXITCODE
Write-Host $out

$evidence = Join-Path $root "$OwnedPath\automate-acceptance.md"
if ($code -eq 0 -and (Test-Path $evidence)) {
  Write-Host "PASS N3 Automate dogfood (exit 0 + evidence file)"
  Get-Content $evidence | Select-Object -First 20
  exit 0
}

if ($code -eq 0 -and -not (Test-Path $evidence)) {
  Write-Host "FAIL exit 0 but missing $evidence — model did not write scope"
  exit 2
}

if ($out -match "HTTP 401|not configured|cannot reach|request failed") {
  Write-Host "FAIL provider/auth — run scripts/dogfood-providers.ps1 first"
  exit 3
}

Write-Host "FAIL N3 exit=$code"
exit $code
