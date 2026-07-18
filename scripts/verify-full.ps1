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

& (Join-Path $PSScriptRoot "verify-quick.ps1") -Root $root
if ($LASTEXITCODE -ne 0) {
    throw "verify-quick failed with exit code $LASTEXITCODE"
}

Push-Location $root
try {
    Invoke-Checked "workspace tests" {
        cargo test --workspace --exclude ade-desktop-app
    }
    Invoke-Checked "ADE CLI build" { cargo build -p ade-cli }

    Push-Location (Join-Path $root "apps/desktop")
    try {
        Invoke-Checked "desktop production build" { npm run build }
    } finally {
        Pop-Location
    }

    Write-Host "==> local API integration smoke"
    $metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) {
        throw "cargo metadata failed with exit code $LASTEXITCODE"
    }
    $executable = Join-Path $metadata.target_directory "debug/ade.exe"
    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
        throw "ADE CLI executable not found at $executable"
    }

    $probe = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )
    $probe.Start()
    $port = ([System.Net.IPEndPoint]$probe.LocalEndpoint).Port
    $probe.Stop()

    $stdout = Join-Path ([System.IO.Path]::GetTempPath()) "ade-api-$PID.stdout.log"
    $stderr = Join-Path ([System.IO.Path]::GetTempPath()) "ade-api-$PID.stderr.log"
    $smokeToken = "ade-verify-$PID"
    $previousToken = $env:ADE_API_TOKEN
    $env:ADE_API_TOKEN = $smokeToken
    try {
        $server = Start-Process `
            -FilePath $executable `
            -ArgumentList @("serve", "--bind", "127.0.0.1:$port") `
            -WorkingDirectory $root `
            -RedirectStandardOutput $stdout `
            -RedirectStandardError $stderr `
            -PassThru
    } finally {
        $env:ADE_API_TOKEN = $previousToken
    }

    try {
        $ready = $false
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ($server.HasExited) {
                $errorText = if (Test-Path $stderr) {
                    Get-Content -LiteralPath $stderr -Raw
                } else {
                    "no stderr captured"
                }
                throw "ADE API exited before readiness: $errorText"
            }
            try {
                $health = Invoke-RestMethod `
                    -Uri "http://127.0.0.1:$port/health/ready" `
                    -TimeoutSec 1
                if ($health.status -eq "ready") {
                    $ready = $true
                    break
                }
            } catch {
                Start-Sleep -Milliseconds 250
            }
        }
        if (-not $ready) {
            throw "ADE API did not become ready on port $port"
        }

        $snapshot = Invoke-RestMethod `
            -Uri "http://127.0.0.1:$port/api/state" `
            -Headers @{ Authorization = "Bearer $smokeToken" } `
            -TimeoutSec 5
        if ($snapshot.schema -ne "ade.api.snapshot/v1") {
            throw "unexpected API snapshot schema: $($snapshot.schema)"
        }
        if ($snapshot.workspace_root -ne $root) {
            throw "API workspace mismatch: $($snapshot.workspace_root)"
        }
        Write-Host "API snapshot passed: score $($snapshot.audit.score)/$($snapshot.audit.score_max)"
    } finally {
        if (-not $server.HasExited) {
            Stop-Process -Id $server.Id -Force
            $server.WaitForExit()
        }
        Remove-Item -LiteralPath $stdout, $stderr -Force -ErrorAction SilentlyContinue
    }
} finally {
    Pop-Location
}

Write-Host "verify-full passed"
