# Continuity dogfood: PDF extract context + MCP search intent
# Prepares .ade/inbox extract + PDF fixture, then thrift resume with owned-path evidence.
# Usage:
#   pwsh -File scripts/dogfood-continuity-pdf-mcp.ps1
# Optional: -Provider opencode -Model deepseek-v4-flash-free -MaxSteps 28
param(
  [string]$Provider = "opencode",
  [string]$BaseUrl = "https://opencode.ai/zen/v1",
  [string]$Model = "deepseek-v4-flash-free",
  [string]$OwnedPath = ".ade/dogfood",
  [int]$MaxSteps = 28
)

$ErrorActionPreference = "Continue"
$root = "C:\Dev\ade"
Set-Location $root

$ade = if (Test-Path "C:\Dev\ade-target\debug\ade.exe") {
  "C:\Dev\ade-target\debug\ade.exe"
} else {
  "ade"
}

Write-Host "=== Continuity PDF + MCP dogfood ==="
Write-Host "Root: $root"
Write-Host "ADE:  $ade"
Write-Host "Prov: $Provider / $Model"
Write-Host ""

if (-not (Test-Path (Join-Path $root "AGENTS.md"))) {
  Write-Host "FAIL missing AGENTS.md — attach ADE repo first"
  exit 1
}

$inbox = Join-Path $root ".ade\inbox"
New-Item -ItemType Directory -Force -Path $inbox | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $root $OwnedPath) | Out-Null

# Minimal text PDF (Helvetica) — Desktop Extract produces the same *.extract.md shape.
$pdfPath = Join-Path $inbox "continuity-dogfood.pdf"
$pdf = @"
%PDF-1.4
1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj
2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj
3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources<< /Font<< /F1 5 0 R >> >> >>endobj
4 0 obj<< /Length 68 >>stream
BT /F1 14 Tf 40 120 Td (ADE Continuity PDF dogfood) Tj ET
endstream
endobj
5 0 obj<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000266 00000 n 
0000000386 00000 n 
trailer<< /Size 6 /Root 1 0 R >>
startxref
465
%%EOF
"@
[System.IO.File]::WriteAllText($pdfPath, $pdf.Replace("`r`n", "`n"))

$extractPath = Join-Path $inbox "continuity-dogfood.extract.md"
$extractBody = @"
# PDF extract

Source: continuity-dogfood.pdf
Path: $pdfPath
Pages: 1 of 1

---

## Page 1

ADE Continuity PDF dogfood

"@
Set-Content -Path $extractPath -Value $extractBody -Encoding utf8

# Office extract shape (Desktop Extract chip writes the same *.extract.md pattern).
$officeExtractPath = Join-Path $inbox "continuity-dogfood-office.extract.md"
$officeExtractBody = @"
# Office extract (docx)

Source: continuity-dogfood.docx
Path: .ade/inbox/continuity-dogfood.docx
Scope: 1 paragraph

---

ADE Continuity Office dogfood

"@
Set-Content -Path $officeExtractPath -Value $officeExtractBody -Encoding utf8

Write-Host "Prepared inbox PDF + extracts:"
Write-Host "  $pdfPath"
Write-Host "  $extractPath"
Write-Host "  $officeExtractPath"
Write-Host ""
Write-Host "Desktop tip: Setup > Integrations > Add GitHub/Linear MCP (token in vault),"
Write-Host "  then Continue on Continuity with this extract attached."
  Write-Host "  Docs: docs/guides/mcp-recipes.md"
Write-Host "  CLI dogfood: mcp=skipped means Continuity+extract only (not full MCP search)."
Write-Host ""

$latestPath = Join-Path $root ".ade\handoff\latest.json"
if (-not (Test-Path $latestPath)) {
  Write-Host "FAIL no .ade/handoff/latest.json — run an agent turn first"
  exit 1
}

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
Write-Host ""

$stamp = (Get-Date).ToUniversalTime().ToString("o")
$extractRel = ".ade/inbox/continuity-dogfood.extract.md"

if ($hostRan) {
  $hostLine = "Host already ran next_safe_command: ``$next``."
} else {
  $hostLine = "Do next_safe_command first: ``$next``."
}

$agentPrompt = @"
Continuity PDF + MCP dogfood (raised tool budget).

Prior handoff status: $status.
$hostLine

Read ``$extractRel`` (PDF extract fixture). Quote one short phrase from it in your evidence.

If MCP tools are available this turn (e.g. GitHub or Linear search), call ONE search-style MCP tool and note the tool name + whether it returned results. If no MCP tools are connected, write ``mcp=skipped (not connected)`` — do not invent tool results.

Using fs__write_file, create or update ONLY ``$OwnedPath/continuity-pdf-mcp-acceptance.md`` with:
- ISO time: $stamp
- continuity pdf+mcp dogfood = pass
- extract_path=$extractRel
- extract_quote: <phrase from extract>
- mcp=<tool name + outcome OR skipped>
- max_steps=$MaxSteps
- next_safe_command=$next
- host_ran_next=$hostRan
- prior_handoff_status=$status
- one sentence: Continuity thrift resume used PDF extract context without pasting prior chat

Do not paste prior chat. Do not edit crates/, apps/, docs/, or run long shell discovery loops.
Do not rebuild binaries. Prefer extract read (+ optional MCP) then one write then stop.
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

$evidence = Join-Path $root "$OwnedPath\continuity-pdf-mcp-acceptance.md"
if (-not (Test-Path $evidence)) {
  Write-Host "FAIL missing evidence $evidence"
  exit 1
}

$evidenceText = Get-Content $evidence -Raw
Write-Host ""
Write-Host "-- evidence --"
Get-Content $evidence | ForEach-Object { Write-Host $_ }

if ($code -ne 0) {
  Write-Host "FAIL agent exit=$code (evidence present but turn failed)"
  exit $code
}

if ($evidenceText -notmatch "continuity-dogfood\.extract\.md|ADE Continuity PDF") {
  Write-Host "FAIL evidence does not clearly cite the PDF extract"
  exit 1
}

Write-Host ""
Write-Host "PASS Continuity PDF dogfood — evidence under $OwnedPath/continuity-pdf-mcp-acceptance.md"
exit 0
