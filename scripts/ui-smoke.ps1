# ADE Desktop UI smoke (Playwright) — layout / IA regressions on Vite preview.
# Does not replace verify ladder G0–G5 (cargo). Tauri IPC is not exercised here.
# Usage: pwsh -File scripts/ui-smoke.ps1
param(
  [string]$Root = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"
$desktop = Join-Path $Root "apps\desktop"
Set-Location $desktop

if (-not (Test-Path "node_modules\@playwright\test")) {
  Write-Host "==> npm install (playwright)"
  npm install
}

Write-Host "==> ensure Chromium for Playwright"
npx playwright install chromium

Write-Host "==> production build (preview serves dist)"
npm run build

Write-Host "==> Playwright e2e (sidebar IA)"
npm run test:e2e
if ($LASTEXITCODE -ne 0) {
  throw "ui-smoke failed with exit code $LASTEXITCODE"
}

Write-Host "ui-smoke OK"
