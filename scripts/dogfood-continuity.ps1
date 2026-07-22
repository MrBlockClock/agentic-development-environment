# N4 Dogfood: Continuity resume (Continue last handoff) with raised tool-round budget
# Usage:
#   pwsh -File scripts/dogfood-continuity.ps1
# Optional: -Provider opencode -Model deepseek-v4-flash-free -MaxSteps 24
param(
  [string]$Provider = "opencode",
  [string]$BaseUrl = "https://opencode.ai/zen/v1",
  [string]$Model = "deepseek-v4-flash-free",
  [string]$OwnedPath = ".ade/dogfood",
  [int]$MaxSteps = 24
)

$ErrorActionPreference = "Continue"
$root = "C:\Dev\ade"
Set-Location $root

$ade = if (Test-Path "C:\Dev\ade-target\debug\ade.exe") {
  "C:\Dev\ade-target\debug\ade.exe"
} else {
  "ade"
}

Write-Host "=== N4 Dogfood Continuity ==="
Write-Host "Root: $root"
Write-Host "ADE:  $ade"
Write-Host "Prov: $Provider / $Model"
Write-Host "Steps: $MaxSteps"
Write-Host "Scope: $OwnedPath"
Write-Host ""

if (-not (Test-Path (Join-Path $root "AGENTS.md"))) {
  Write-Host "FAIL missing AGENTS.md — attach ADE repo first"
  exit 1
}

$latestPath = Join-Path $root ".ade\handoff\latest.json"
if (-not (Test-Path $latestPath)) {
  Write-Host "FAIL no .ade/handoff/latest.json — run an agent turn first"
  exit 1
}

$capsule = Get-Content $latestPath -Raw | ConvertFrom-Json
$next = if ($capsule.next_safe_command) { [string]$capsule.next_safe_command } else { "ade audit" }
$status = if ($capsule.turn_status) { [string]$capsule.turn_status } else { "unknown" }
$blocker = ""
if ($capsule.blockers -and $capsule.blockers.Count -gt 0) {
  $blocker = [string]$capsule.blockers[0]
}

Write-Host "-- latest handoff --"
Write-Host "  status:   $status"
Write-Host "  next:     $next"
if ($blocker) { Write-Host "  blocker:  $blocker" }
Write-Host ""

Write-Host "-- next_safe_command (host) --"
$env:RUST_LOG = "error"
# Prefer the documented Continuity first step; audit may be slow — allow verify G0 as fallback.
$hostCmd = $next.Trim()
if ($hostCmd -match "^ade\s+") {
  $parts = $hostCmd -split "\s+"
  $adeArgs = $parts[1..($parts.Length - 1)]
  & $ade @adeArgs 2>&1 | Select-Object -Last 25 | ForEach-Object { Write-Host $_ }
  Write-Host "  host exit=$LASTEXITCODE"
} else {
  Write-Host "  skip non-ade next_safe_command: $hostCmd"
}
Write-Host ""

New-Item -ItemType Directory -Force -Path (Join-Path $root $OwnedPath) | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString("o")

$prompt = @"
N4 Continuity dogfood (raised tool budget).

Host already ran next_safe_command: ``$next`` (status was $status).
$(if ($blocker) { "Prior blocker: $blocker" } else { "No prior blockers." })

Using fs__write_file, create or update ONLY ``$OwnedPath/continuity-acceptance.md`` with:
- ISO time: $stamp
- continuity resume dogfood = pass
- max_steps=$MaxSteps
- next_safe_command=$next
- prior_handoff_status=$status
- one sentence: Continuity resume completed under owned path $OwnedPath

Do not edit crates/, apps/, docs/, or run long shell discovery loops.
Do not rebuild binaries. Prefer one write then stop.
"@

Write-Host "-- agent turn (act + owned paths + max-steps $MaxSteps) --"
$out = & $ade agent `
  --provider $Provider `
  --base-url $BaseUrl `
  --model $Model `
  --autonomy act `
  --approve-owned-paths `
  --owned-path $OwnedPath `
  --max-steps $MaxSteps `
  --input-cost-per-mtok 0 `
  --output-cost-per-mtok 0 `
  $prompt 2>&1 | Out-String

$code = $LASTEXITCODE
Write-Host $out
Write-Host "agent exit=$code"

$evidence = Join-Path $root "$OwnedPath\continuity-acceptance.md"
if (-not (Test-Path $evidence)) {
  Write-Host "FAIL missing evidence $evidence"
  exit 1
}

Write-Host ""
Write-Host "-- evidence --"
Get-Content $evidence | ForEach-Object { Write-Host $_ }

$latest2 = Get-Content $latestPath -Raw | ConvertFrom-Json
Write-Host ""
Write-Host "-- handoff after --"
Write-Host "  status: $($latest2.turn_status)"
Write-Host "  model:  $($latest2.provider)/$($latest2.model)"

if ($code -ne 0) {
  Write-Host "FAIL agent turn exit $code"
  exit $code
}

Write-Host ""
Write-Host "PASS Continuity dogfood — evidence under $OwnedPath/continuity-acceptance.md"
exit 0
