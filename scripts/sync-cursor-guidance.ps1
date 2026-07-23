# Sync .ade guidance → Cursor mirrors
#
# Authoritative trees: .ade/rules, .ade/skills
# Cursor mirrors:      .cursor/rules, .cursor/skills
#
# Usage (from repo root):
#   pwsh -File scripts/sync-cursor-guidance.ps1
#   pwsh -File scripts/sync-cursor-guidance.ps1 -Check   # exit 1 if drift

param(
  [switch]$Check
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$pairs = @(
  @{ Src = Join-Path $root ".ade\rules";  Dst = Join-Path $root ".cursor\rules" },
  @{ Src = Join-Path $root ".ade\skills"; Dst = Join-Path $root ".cursor\skills" }
)

function Get-RelFiles([string]$dir) {
  if (-not (Test-Path $dir)) { return @() }
  Get-ChildItem $dir -Recurse -File | ForEach-Object {
    $_.FullName.Substring($dir.Length).TrimStart('\', '/')
  }
}

$drift = @()
foreach ($p in $pairs) {
  if (-not (Test-Path $p.Src)) {
    Write-Error "Missing source: $($p.Src)"
  }
  if (-not $Check) {
    New-Item -ItemType Directory -Force -Path $p.Dst | Out-Null
    # Remove destination files not in source (keep trees identical)
    $srcSet = [System.Collections.Generic.HashSet[string]]::new([string[]](Get-RelFiles $p.Src))
    foreach ($rel in (Get-RelFiles $p.Dst)) {
      if (-not $srcSet.Contains($rel)) {
        Remove-Item (Join-Path $p.Dst $rel) -Force
      }
    }
    Copy-Item -Path (Join-Path $p.Src "*") -Destination $p.Dst -Recurse -Force
    Write-Host "Synced $($p.Src) -> $($p.Dst)"
  } else {
    foreach ($rel in (Get-RelFiles $p.Src)) {
      $a = Join-Path $p.Src $rel
      $b = Join-Path $p.Dst $rel
      if (-not (Test-Path $b)) {
        $drift += "missing mirror: $rel"
        continue
      }
      if ((Get-FileHash $a).Hash -ne (Get-FileHash $b).Hash) {
        $drift += "content drift: $rel"
      }
    }
    foreach ($rel in (Get-RelFiles $p.Dst)) {
      if (-not (Test-Path (Join-Path $p.Src $rel))) {
        $drift += "extra mirror: $rel"
      }
    }
  }
}

if ($Check) {
  if ($drift.Count -gt 0) {
    $drift | ForEach-Object { Write-Host $_ }
    exit 1
  }
  Write-Host "OK: .cursor mirrors match .ade"
}
