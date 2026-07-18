param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"
$resolvedRoot = (Resolve-Path -LiteralPath $Root).Path

function Read-Version([string]$Command, [string[]]$Arguments) {
    try {
        $value = & $Command @Arguments 2>$null
        if ($LASTEXITCODE -eq 0) {
            return ($value | Select-Object -First 1).ToString().Trim()
        }
    } catch {
        return $null
    }
    return $null
}

$branch = $null
try {
    $branch = (& git -C $resolvedRoot branch --show-current 2>$null).Trim()
} catch {
    $branch = $null
}

[ordered]@{
    schema = "ade.environment.probe/v1"
    root = $resolvedRoot
    os = [System.Environment]::OSVersion.VersionString
    architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    shell = "PowerShell $($PSVersionTable.PSVersion)"
    git_branch = $branch
    rustc = Read-Version "rustc" @("--version")
    cargo = Read-Version "cargo" @("--version")
    node = Read-Version "node" @("--version")
    agents_contract = Test-Path -LiteralPath (Join-Path $resolvedRoot "AGENTS.md") -PathType Leaf
} | ConvertTo-Json -Depth 3
