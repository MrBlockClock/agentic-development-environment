# G4 Dogfood: Isolate Apply on a claimed task (worktree provision + isolation honesty)
# Usage:
#   pwsh -File scripts/dogfood-isolate-apply.ps1
#   pwsh -File scripts/dogfood-isolate-apply.ps1 -Live   # also run worker --once --worktree
# Optional: -Provider opencode -Model deepseek-v4-flash-free
param(
  [string]$Provider = "opencode",
  [string]$BaseUrl = "https://opencode.ai/zen/v1",
  [string]$Model = "deepseek-v4-flash-free",
  [string]$OwnedPath = ".ade/dogfood/isolate",
  [switch]$Live
)

$ErrorActionPreference = "Stop"
$root = "C:\Dev\ade"
Set-Location $root

$ade = if (Test-Path "C:\Dev\ade-target\debug\ade.exe") {
  "C:\Dev\ade-target\debug\ade.exe"
} else {
  "ade"
}

Write-Host "=== G4 Dogfood Isolate Apply ==="
Write-Host "Root: $root"
Write-Host "ADE:  $ade"
Write-Host "Live: $Live"
Write-Host "Scope: $OwnedPath"
Write-Host ""

if (-not (Test-Path (Join-Path $root "AGENTS.md"))) {
  Write-Host "FAIL missing AGENTS.md — attach ADE repo first"
  exit 1
}

New-Item -ItemType Directory -Force -Path (Join-Path $root $OwnedPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $root ".ade\worktrees") | Out-Null

Write-Host "-- clear stale isolate tasks --"
try {
  $raw = & $ade task list --json 2>$null | Out-String
  # Strip leading log lines if present; keep from first '['.
  $idx = $raw.IndexOf('[')
  if ($idx -ge 0) {
    $tasks = ($raw.Substring($idx) | ConvertFrom-Json)
    foreach ($t in $tasks) {
      $paths = @($t.owned_paths)
      if ($paths -notcontains $OwnedPath) { continue }
      if ($t.status -in @('queued', 'claimed', 'running')) {
        Write-Host "  cancel $($t.id) ($($t.status))"
        & $ade task cancel $t.id --approve 2>$null | Out-Null
      }
    }
  }
} catch {
  Write-Host "  (skip stale cleanup: $_)"
}

$agentId = [guid]::NewGuid().ToString()
$goal = "G4 isolate dogfood: write ONLY under $OwnedPath/acceptance.md noting isolated worktree Apply."

Write-Host "-- enqueue task --"
$enqueueOut = & $ade task enqueue --goal $goal --path $OwnedPath --approve 2>&1 | Out-String
Write-Host $enqueueOut
if ($LASTEXITCODE -ne 0) {
  Write-Host "FAIL enqueue"
  exit 1
}

Write-Host "-- claim + start --"
$claimOut = & $ade task claim --agent $agentId --approve 2>&1 | Out-String
Write-Host $claimOut
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL claim"; exit 1 }
$claimIdx = $claimOut.IndexOf('{')
if ($claimIdx -lt 0) { Write-Host "FAIL claim JSON missing"; exit 1 }
$claimed = ($claimOut.Substring($claimIdx) | ConvertFrom-Json)
$taskId = [string]$claimed.id
if (-not $taskId) { Write-Host "FAIL empty claimed task id"; exit 1 }
Write-Host "Task: $taskId"
Write-Host "Agent: $agentId"
& $ade task start $taskId --agent $agentId --approve | Out-Null
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL start"; exit 1 }

$wtPath = Join-Path $root ".ade\worktrees\$taskId"
$branch = "ade/task-$($taskId.Substring(0, 8))"

Write-Host "-- provision worktree (Desktop Isolate mirror) --"
Write-Host "Path: $wtPath"
Write-Host "Branch: $branch"
& $ade worktree add --path $wtPath --branch $branch --approve
if ($LASTEXITCODE -ne 0) {
  Write-Host "FAIL worktree add"
  & $ade task fail $taskId --agent $agentId --reason "worktree provision failed" --approve 2>$null
  exit 1
}

if (-not (Test-Path $wtPath)) {
  Write-Host "FAIL worktree path missing after add"
  exit 1
}

$wtList = & $ade worktree list 2>&1 | Out-String
Write-Host $wtList
if ($wtList -notmatch [regex]::Escape($taskId)) {
  Write-Host "FAIL worktree list does not mention task id"
  exit 1
}

# Isolation honesty: write only inside the worktree owned path.
$wtOwned = Join-Path $wtPath $OwnedPath
New-Item -ItemType Directory -Force -Path $wtOwned | Out-Null
$marker = Join-Path $wtOwned "acceptance.md"
$stamp = (Get-Date).ToString("o")
@"
# Isolate Apply dogfood

- at: $stamp
- task: $taskId
- worktree: $wtPath
- owned: $OwnedPath
- note: writes landed in worktree; primary checkout must not have this file until merge
"@ | Set-Content -Path $marker -Encoding utf8

$primaryMarker = Join-Path $root "$OwnedPath\acceptance.md"
if (Test-Path $primaryMarker) {
  Write-Host "FAIL primary checkout already has $OwnedPath/acceptance.md — isolation unclear"
  exit 1
}
if (-not (Test-Path $marker)) {
  Write-Host "FAIL marker missing in worktree"
  exit 1
}
Write-Host "OK isolation: marker in worktree only"

Write-Host "-- complete task + cleanup worktree --"
& $ade task complete $taskId --agent $agentId --approve
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL complete"; exit 1 }
& $ade worktree remove --path $wtPath --approve --force
if ($LASTEXITCODE -ne 0) {
  Write-Host "FAIL worktree remove"
  exit 1
}
if (Test-Path $wtPath) {
  Write-Host "FAIL worktree still present after remove"
  exit 1
}
Write-Host "OK worktree cleaned"

$liveOk = $null
if ($Live) {
  Write-Host "-- live worker --once --worktree --"
  $liveGoal = "G4 live Isolate: create/update $OwnedPath/live-acceptance.md via fs__write_file with ISO time and worktree note. Do not edit crates/ or apps/."
  $liveEnqueue = & $ade task enqueue --goal $liveGoal --path $OwnedPath --approve 2>&1 | Out-String
  Write-Host $liveEnqueue
  if ($LASTEXITCODE -ne 0) {
    Write-Host "FAIL live enqueue"
    exit 1
  }
  $env:RUST_LOG = "error"
  & $ade worker run `
    --agent ([guid]::NewGuid().ToString()) `
    --provider $Provider `
    --base-url $BaseUrl `
    --model $Model `
    --worktree `
    --cleanup-worktree `
    --once `
    --approve `
    --input-cost-per-mtok 0.01 `
    --output-cost-per-mtok 0.01
  $liveCode = $LASTEXITCODE
  if ($liveCode -ne 0) {
    Write-Host "WARN live worker exited $liveCode (harness path still counts)"
    $liveOk = $false
  } else {
    Write-Host "OK live worker --once --worktree"
    $liveOk = $true
  }
}

$summary = Join-Path $root ".ade\dogfood\isolate-acceptance.md"
New-Item -ItemType Directory -Force -Path (Split-Path $summary) | Out-Null
$liveLine = if ($null -eq $liveOk) { "skipped" } elseif ($liveOk) { "pass" } else { "warn/fail" }
@"
# G4 Isolate Apply acceptance

- at: $stamp
- task: $taskId
- harness: provision → write-in-worktree-only → complete → remove
- live: $liveLine
- script: scripts/dogfood-isolate-apply.ps1
"@ | Set-Content -Path $summary -Encoding utf8

Write-Host ""
Write-Host "PASS isolate dogfood"
Write-Host "Evidence: $summary"
exit 0
