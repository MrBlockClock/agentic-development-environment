# Continuity thrift dogfood: host next_safe → thrift resume → owned-path evidence
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

Write-Host "=== Continuity thrift dogfood ==="
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

# Dogfood uses $0 rates; default session/daily caps still apply — allow unpriced.
$env:ADE_ALLOW_UNPRICED = "1"
$env:RUST_LOG = "error"
Write-Host "-- ade handoff resume (host next_safe + thrift prompt) --"
$resumeJson = & $ade handoff resume --json 2>&1 | Out-String
if ($LASTEXITCODE -ne 0) {
  Write-Host $resumeJson
  Write-Host "FAIL ade handoff resume exit $LASTEXITCODE"
  exit $LASTEXITCODE
}

try {
  $resume = $resumeJson | ConvertFrom-Json
} catch {
  Write-Host $resumeJson
  Write-Host "FAIL resume JSON parse"
  exit 1
}

if (-not $resume.available) {
  Write-Host "FAIL handoff resume not available"
  exit 1
}

$next = [string]$resume.nextSafeCommand
if (-not $next) { $next = [string]$resume.next_safe_command }
if (-not $next) { $next = "ade audit" }

$status = [string]$resume.turnStatus
if (-not $status) { $status = [string]$resume.turn_status }
if (-not $status) { $status = "unknown" }

$hostRan = $false
if ($null -ne $resume.hostRanNext) { $hostRan = [bool]$resume.hostRanNext }
elseif ($null -ne $resume.host_ran_next) { $hostRan = [bool]$resume.host_ran_next }

$prompt = [string]$resume.resumePrompt
if (-not $prompt) { $prompt = [string]$resume.resume_prompt }

Write-Host "  status:   $status"
Write-Host "  next:     $next"
Write-Host "  hostRan:  $hostRan"
if ($prompt -notmatch "Do not paste prior chat") {
  Write-Host "FAIL thrift resume missing no-paste guard"
  exit 1
}
if ($prompt -notmatch "\.ade/continuity/last-write\.json") {
  Write-Host "WARN resume does not mention last-write.json (continuing)"
}
Write-Host ""

New-Item -ItemType Directory -Force -Path (Join-Path $root $OwnedPath) | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString("o")

if ($hostRan) {
  $hostLine = "Host already ran next_safe_command: ``$next``."
} else {
  $hostLine = "Do next_safe_command first: ``$next``."
}

$agentPrompt = @"
Continuity thrift dogfood (raised tool budget).

Prior handoff status: $status.
$hostLine

Using fs__write_file, create or update ONLY ``$OwnedPath/continuity-acceptance.md`` with:
- ISO time: $stamp
- continuity thrift dogfood = pass
- max_steps=$MaxSteps
- next_safe_command=$next
- host_ran_next=$hostRan
- prior_handoff_status=$status
- one sentence: Continuity thrift resume completed under owned path $OwnedPath without pasting prior chat

Do not paste prior chat. Do not edit crates/, apps/, docs/, or run long shell discovery loops.
Do not rebuild binaries. Prefer one write then stop.
"@

Write-Host "-- agent turn (act + owned paths + max-steps $MaxSteps) --"
$env:RUST_LOG = "error"
$out = & $ade agent `
  --provider $Provider `
  --base-url $BaseUrl `
  --model $Model `
  --autonomy act `
  --approve-owned-paths `
  --owned-path $OwnedPath `
  --max-steps $MaxSteps `
  --input-cost-per-mtok 0.01 `
  --output-cost-per-mtok 0.01 `
  $agentPrompt 2>&1 | Out-String

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

$lastWrite = Join-Path $root ".ade\continuity\last-write.json"
if (Test-Path $lastWrite) {
  Write-Host ""
  Write-Host "-- last-write --"
  & $ade handoff last-write 2>&1 | Select-Object -First 20 | ForEach-Object { Write-Host $_ }
}

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
Write-Host "PASS Continuity thrift dogfood — evidence under $OwnedPath/continuity-acceptance.md"
exit 0
