# Dogfood: verify ADE provider vault + live turns (no secrets printed).
# Usage: pwsh -File scripts/dogfood-providers.ps1
$ErrorActionPreference = "Continue"
$ade = if (Test-Path "C:\Dev\ade-target\debug\ade.exe") {
  "C:\Dev\ade-target\debug\ade.exe"
} else {
  "ade"
}
Set-Location C:\Dev\ade
$env:RUST_LOG = "error"

function Show-Status([string]$Provider) {
  $line = & $ade keys status --profile local $Provider 2>&1 | Out-String
  if ($line -match "is configured") { "OK  $Provider vault" }
  elseif ($line -match "not configured") { "MISS $Provider vault" }
  else { "??  $Provider : $($line.Trim())" }
}

function Test-Turn([string]$Provider, [string]$BaseUrl, [string]$Model, [string]$Expect) {
  Write-Host "-- $Provider / $Model"
  $out = & $ade agent --provider $Provider --base-url $BaseUrl --model $Model `
    --autonomy observe --max-steps 1 --input-cost-per-mtok 0 --output-cost-per-mtok 0 `
    "Reply with exactly: $Expect" 2>&1 | Out-String
  $code = $LASTEXITCODE
  if ($code -eq 0 -and $out -match [regex]::Escape($Expect)) {
    "PASS $Provider"
  } elseif ($out -match "HTTP 401") {
    "FAIL $Provider auth (401) — key invalid for this gateway"
  } elseif ($out -match "cannot reach|request failed|error sending") {
    "FAIL $Provider unreachable — start local gateway?"
  } else {
    "FAIL $Provider exit=$code"
    ($out -split "`n" | Where-Object { $_ -match "Error|error|HTTP" } | Select-Object -First 3) -join "`n"
  }
}

Write-Host "=== Vault ==="
Show-Status opencode
Show-Status freellm

Write-Host "`n=== Live turns ==="
Test-Turn opencode "https://opencode.ai/zen/v1" "deepseek-v4-flash-free" "zen-ok"
Test-Turn freellm "http://127.0.0.1:3001/v1" "auto" "freellm-ok"

Write-Host "`n=== Local FreeLLMAPI ==="
try {
  $r = Invoke-WebRequest "http://127.0.0.1:3001/api/ping" -UseBasicParsing -TimeoutSec 3
  "OK  freellmapi ping $($r.StatusCode)"
} catch {
  "MISS freellmapi — start FreeLLMAPI Desktop or: docker compose -f ~/freellmapi/docker-compose.yml up -d"
}

Remove-Item Env:RUST_LOG -ErrorAction SilentlyContinue
