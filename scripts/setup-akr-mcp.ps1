<#
.SYNOPSIS
    One-time setup for the AKR MCP server across Codex, OpenCode, and Claude on Windows.

.DESCRIPTION
    PowerShell mirror of scripts/setup-akr-mcp.sh. Builds (or reuses) the AKR release
    binaries, installs them to $HOME\.local\bin, and registers the AKR MCP server with:
      - Codex   (~/.codex/config.toml)
      - OpenCode (~/.config/opencode/opencode.jsonc)
      - Claude  (via `claude mcp add --scope user`)

.PARAMETER RepoDir
    AKR repo root (default: parent directory of this script's directory).

.PARAMETER DryRun
    Print changes without writing.

.PARAMETER UseDebug
    Use target/debug binaries instead of target/release.

.PARAMETER NoClaude
    Skip Claude registration.

.PARAMETER NoCodex
    Skip Codex config update.

.PARAMETER NoOpenCode
    Skip OpenCode config update.
#>

[CmdletBinding()]
param(
    [string]$RepoDir,
    [switch]$DryRun,
    [Alias("Debug_")]
    [switch]$UseDebug,
    [switch]$NoClaude,
    [switch]$NoCodex,
    [switch]$NoOpenCode,
    [switch]$Help
)

function Show-Usage {
    @"
Usage: setup-akr-mcp.ps1 [-RepoDir DIR] [-DryRun] [-UseDebug] [-NoClaude] [-NoCodex] [-NoOpenCode]

One-time setup for the AKR MCP server across:
- AKR CLI build/install
- Codex MCP config (~\.codex\config.toml)
- OpenCode MCP config (~\.config\opencode\opencode.jsonc)
- Claude MCP registration

Options:
  -RepoDir DIR     AKR repo root (default: script's parent directory)
  -DryRun          Print changes without writing
  -UseDebug        Use target\debug\akr-mcp.exe instead of release
  -NoClaude        Skip Claude registration
  -NoCodex         Skip Codex config update
  -NoOpenCode      Skip OpenCode config update
  -Help            Show this help
"@
}

if ($Help) {
    Show-Usage
    exit 0
}

$ErrorActionPreference = "Stop"

if (-not $RepoDir) {
    $RepoDir = Split-Path -Parent $PSScriptRoot
}
$RepoDir = (Resolve-Path $RepoDir).Path

function Log([string]$msg) {
    Write-Host "[setup-akr-mcp] $msg"
}

function Invoke-Step {
    param([string]$Description, [scriptblock]$Action)
    if ($DryRun) {
        Log "DRY-RUN: $Description"
    } else {
        & $Action
    }
}

$CargoToml = Join-Path $RepoDir "Cargo.toml"
if (-not (Test-Path $CargoToml)) {
    Write-Error "error: no Cargo.toml at repo root: $CargoToml"
    exit 1
}

$AkrMcpCrate = Join-Path $RepoDir "crates\akr-mcp"
if (-not (Test-Path $AkrMcpCrate)) {
    Write-Error "error: akr-mcp crate missing at $AkrMcpCrate"
    exit 1
}

$AkrBinDir = Join-Path $HOME ".local\bin"
$AkrExeDest = Join-Path $AkrBinDir "akr.exe"
$AkrMcpExeDest = Join-Path $AkrBinDir "akr-mcp.exe"

# Build (or reuse) AKR binaries
if ($UseDebug) {
    $BuildMode = "debug"
    $BuildCmdDisplay = "cargo build --package akr-cli --package akr-mcp"
    $SourceAkr = Join-Path $RepoDir "target\debug\akr.exe"
    $SourceAkrMcp = Join-Path $RepoDir "target\debug\akr-mcp.exe"
} else {
    $BuildMode = "release"
    $BuildCmdDisplay = "cargo build --release --package akr-cli --package akr-mcp"
    $SourceAkr = Join-Path $RepoDir "target\release\akr.exe"
    $SourceAkrMcp = Join-Path $RepoDir "target\release\akr-mcp.exe"
}

Log "Using repo: $RepoDir"
Log "Build mode: $BuildMode"

if (-not ((Test-Path $SourceAkr) -and (Test-Path $SourceAkrMcp))) {
    Invoke-Step "$BuildCmdDisplay (in $RepoDir)" {
        Push-Location $RepoDir
        try {
            if ($UseDebug) {
                cargo build --package akr-cli --package akr-mcp
            } else {
                cargo build --release --package akr-cli --package akr-mcp
            }
            if ($LASTEXITCODE -ne 0) {
                throw "cargo build failed with exit code $LASTEXITCODE"
            }
        } finally {
            Pop-Location
        }
    }
} else {
    Log "Binaries already present, skipping build: $SourceAkr, $SourceAkrMcp"
}

if (-not $DryRun) {
    if (-not (Test-Path $SourceAkr)) {
        Write-Error "error: built binary not found: $SourceAkr`nHint: rerun with -UseDebug or build first manually."
        exit 1
    }
    if (-not (Test-Path $SourceAkrMcp)) {
        Write-Error "error: built binary not found: $SourceAkrMcp`nHint: rerun with -UseDebug or build first manually."
        exit 1
    }
}

Invoke-Step "mkdir -p $AkrBinDir; copy $SourceAkr -> $AkrExeDest; copy $SourceAkrMcp -> $AkrMcpExeDest" {
    New-Item -ItemType Directory -Force -Path $AkrBinDir | Out-Null
    Copy-Item -Path $SourceAkr -Destination $AkrExeDest -Force
    Copy-Item -Path $SourceAkrMcp -Destination $AkrMcpExeDest -Force
}
Log "Installed $AkrExeDest"
Log "Installed $AkrMcpExeDest"

# Install/refresh a section in ~\.codex\config.toml
if (-not $NoCodex) {
    $CodexCfg = Join-Path $HOME ".codex\config.toml"
    if (-not (Test-Path $CodexCfg)) {
        Write-Warning "Codex config not found: $CodexCfg (skipping Codex update)"
    } else {
        $codexText = Get-Content -Raw -LiteralPath $CodexCfg
        # Escape backslashes for a TOML basic (double-quoted) string.
        $escapedMcpPath = $AkrMcpExeDest.Replace('\', '\\')
        if ($codexText -match '(?m)^\[mcp_servers\.akr\]') {
            Log "Codex already has [mcp_servers.akr]; updating command to $AkrMcpExeDest"
            Invoke-Step "update [mcp_servers.akr] command in $CodexCfg" {
                Copy-Item -LiteralPath $CodexCfg -Destination "$CodexCfg.bak" -Force
                $newText = [regex]::Replace(
                    $codexText,
                    '(?m)^\[mcp_servers\.akr\]\r?\ncommand\s*=\s*"[^"]*"',
                    "[mcp_servers.akr]`ncommand = `"$escapedMcpPath`""
                )
                Set-Content -LiteralPath $CodexCfg -Value $newText -NoNewline
            }
        } else {
            Log "Appending [mcp_servers.akr] to Codex config"
            Invoke-Step "append [mcp_servers.akr] to $CodexCfg" {
                Copy-Item -LiteralPath $CodexCfg -Destination "$CodexCfg.bak" -Force
                $needsNewline = -not $codexText.EndsWith("`n")
                $suffix = ""
                if ($needsNewline) { $suffix = "`n" }
                $addition = "$suffix`n[mcp_servers.akr]`ncommand = `"$escapedMcpPath`"`n"
                Add-Content -LiteralPath $CodexCfg -Value $addition -NoNewline
            }
        }
    }
}

# Update ~\.config\opencode\opencode.jsonc
if (-not $NoOpenCode) {
    $OpCfg = Join-Path $HOME ".config\opencode\opencode.jsonc"
    if (-not (Test-Path $OpCfg)) {
        Write-Warning "OpenCode config not found: $OpCfg (skipping OpenCode update)"
    } else {
        Log "Updating OpenCode MCP section"
        $opText = Get-Content -Raw -LiteralPath $OpCfg
        if ($opText -match '/\*' -or $opText -match '(?m)^\s*//') {
            Write-Warning "OpenCode config appears to contain comments (JSONC); attempting a targeted text edit instead of full JSON parse."
            Invoke-Step "targeted-edit .mcp.akr in $OpCfg" {
                Copy-Item -LiteralPath $OpCfg -Destination "$OpCfg.bak" -Force
                $jsonMcpPath = $AkrMcpExeDest.Replace('\', '\\')
                $newSection = "`"akr`": { `"type`": `"local`", `"command`": [ `"$jsonMcpPath`" ], `"enabled`": true }"
                if ($opText -match '"akr"\s*:\s*\{[^}]*\}') {
                    $newText = [regex]::Replace($opText, '"akr"\s*:\s*\{[^}]*\}', { param($m) $newSection })
                    Set-Content -LiteralPath $OpCfg -Value $newText -NoNewline
                } else {
                    Write-Warning "Could not locate an existing 'akr' mcp section to replace and config has comments; manual edit required."
                }
            }
        } else {
            Invoke-Step "set .mcp.akr in $OpCfg (JSON parse/edit)" {
                Copy-Item -LiteralPath $OpCfg -Destination "$OpCfg.bak" -Force
                # Avoid -AsHashtable: not available on Windows PowerShell 5.1.
                $obj = $opText | ConvertFrom-Json
                $akrEntry = [PSCustomObject]@{
                    type    = "local"
                    command = @($AkrMcpExeDest)
                    enabled = $true
                }
                if ($null -eq $obj.mcp) {
                    $obj | Add-Member -MemberType NoteProperty -Name "mcp" -Value ([PSCustomObject]@{}) -Force
                }
                $obj.mcp | Add-Member -MemberType NoteProperty -Name "akr" -Value $akrEntry -Force
                $newJson = $obj | ConvertTo-Json -Depth 100
                Set-Content -LiteralPath $OpCfg -Value $newJson -NoNewline
            }
        }
    }
}

# Register Claude MCP server
if (-not $NoClaude) {
    $claudeCmd = Get-Command claude -ErrorAction SilentlyContinue
    if (-not $claudeCmd) {
        Write-Warning "claude binary not found; skipping Claude registration"
    } else {
        $prevEap = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $null = & claude mcp get akr 2>&1
        $ErrorActionPreference = $prevEap
        if ($LASTEXITCODE -eq 0) {
            Log "Claude already has an AKR MCP server configured"
        } else {
            Log "Registering AKR MCP in Claude (user scope)"
            Invoke-Step "claude mcp add --scope user akr $AkrMcpExeDest" {
                & claude mcp add --scope user akr $AkrMcpExeDest
            }
        }
    }
}

Log "Done. Restart Codex, Claude, and OpenCode to load updated MCP config."
