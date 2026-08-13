<#
.SYNOPSIS
    One-time setup for the AKR MCP server across Codex, OpenCode, and Claude on Windows.

.DESCRIPTION
    PowerShell mirror of scripts/setup-akr-mcp.sh. Builds the AKR release binaries,
    installs them to $HOME\.local\bin, and registers the AKR MCP server with:
      - Codex   (~/.codex/config.toml)
      - OpenCode (~/.config/opencode/opencode.jsonc)
      - Claude  (via `claude mcp add --scope user`)
    It also refreshes the AKR section of the global agent instruction files from
    scripts/agent-section.md.

    Safe to re-run: every step either rewrites in place or is a no-op. Run it after any
    change to the AKR source or to scripts/agent-section.md.

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

.PARAMETER NoAgents
    Skip the AKR section of the global agent instruction files.

.PARAMETER NoBuild
    Install whatever is already in target/ instead of building first. The default is to
    always build: cargo decides what is stale, and it is a no-op when nothing changed.
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
    [switch]$NoAgents,
    [switch]$NoBuild,
    [switch]$Help
)

function Show-Usage {
    @"
Usage: setup-akr-mcp.ps1 [-RepoDir DIR] [-DryRun] [-UseDebug] [-NoClaude] [-NoCodex] [-NoOpenCode] [-NoAgents] [-NoBuild]

One-time setup for the AKR MCP server across:
- AKR CLI build/install
- Codex MCP config (~\.codex\config.toml)
- OpenCode MCP config (~\.config\opencode\opencode.jsonc)
- Claude MCP registration
- The AKR section of the global agent instruction files

Options:
  -RepoDir DIR     AKR repo root (default: script's parent directory)
  -DryRun          Print changes without writing
  -UseDebug        Use target\debug\akr-mcp.exe instead of release
  -NoClaude        Skip Claude registration
  -NoCodex         Skip Codex config update
  -NoOpenCode      Skip OpenCode config update
  -NoAgents        Skip the agent instruction files
  -NoBuild         Install what is already in target/ without building first
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

# Always build. Cargo is the authority on staleness — it is a fast no-op when nothing
# changed, and skipping it because a binary happens to exist is how this script used to
# install yesterday's build over today's source.
if ($NoBuild) {
    Log "Skipping build on request (-NoBuild); installing whatever is in target\$BuildMode"
} else {
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

# Install by renaming the old file out of the way first, rather than writing through it.
#
# Windows locks a running image: `Copy-Item` onto the .exe of a server that is currently
# running fails with "being used by another process" — which is the common case, since the
# stale server you are replacing is the reason you are running this. A rename of a running
# image is allowed, so the running process keeps the file it already opened under its new
# name, and the new build lands on the path it expects. The displaced file is deleted on
# the next run, once nothing holds it.
function Install-Binary {
    param([string]$Source, [string]$Destination)

    $displaced = "$Destination.old"
    if (Test-Path -LiteralPath $displaced) {
        # Best effort: still locked by a process that has not exited yet is fine.
        try { Remove-Item -LiteralPath $displaced -Force -ErrorAction Stop } catch {}
    }
    if (Test-Path -LiteralPath $Destination) {
        if ((Get-FileHash -LiteralPath $Source).Hash -eq (Get-FileHash -LiteralPath $Destination).Hash) {
            Log "Unchanged, leaving in place: $Destination"
            return
        }
        Move-Item -LiteralPath $Destination -Destination $displaced -Force
    }
    try {
        Copy-Item -LiteralPath $Source -Destination $Destination -Force
    } catch {
        # Put the old one back rather than leaving nothing on the path.
        if (Test-Path -LiteralPath $displaced) {
            Move-Item -LiteralPath $displaced -Destination $Destination -Force
        }
        throw
    }
}

Invoke-Step "mkdir -p $AkrBinDir; install $SourceAkr -> $AkrExeDest; install $SourceAkrMcp -> $AkrMcpExeDest" {
    New-Item -ItemType Directory -Force -Path $AkrBinDir | Out-Null
    Install-Binary -Source $SourceAkr -Destination $AkrExeDest
    Install-Binary -Source $SourceAkrMcp -Destination $AkrMcpExeDest
}
Log "Installed $AkrExeDest"
Log "Installed $AkrMcpExeDest"
if (-not $DryRun) {
    $InstalledVersion = & $AkrMcpExeDest --version
    if ($LASTEXITCODE -ne 0) {
        throw "Installed akr-mcp failed its version check: $AkrMcpExeDest"
    }
    Log "Verified installed server: $InstalledVersion ($AkrMcpExeDest)"
}
Log "NOTE: a server that is already running keeps the old binary until it restarts."
Log "      Reconnect the MCP server (or restart the session) before using knowledge.* tools."

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

# Install/refresh the AKR section of the global agent instruction files.
#
# The section lives between HTML comment markers and is rewritten in place, so re-running
# this script updates the guidance instead of stacking another copy of it. Everything
# outside the markers — including sections other tools own, like CodeGraph's — is copied
# through untouched. A file with no markers gets the section appended once.
$AgentSectionFile = Join-Path $RepoDir "scripts\agent-section.md"
$AgentBegin = "<!-- AKR_START -->"
$AgentEnd = "<!-- AKR_END -->"

function Install-AgentSection {
    param([string]$Target)

    $parent = Split-Path -Parent $Target
    if (-not (Test-Path -LiteralPath $parent)) {
        Log "No $parent; skipping $Target"
        return
    }
    if (-not (Test-Path -LiteralPath $AgentSectionFile)) {
        throw "agent section source missing: $AgentSectionFile"
    }
    $section = (Get-Content -Raw -LiteralPath $AgentSectionFile).TrimEnd("`r", "`n")
    $block = "$AgentBegin`n$section`n$AgentEnd"

    $existing = ""
    if (Test-Path -LiteralPath $Target) {
        $existing = Get-Content -Raw -LiteralPath $Target
        if ($null -eq $existing) { $existing = "" }
    }
    $hasMarkers = $existing.Contains($AgentBegin) -and $existing.Contains($AgentEnd)
    $action = if ($hasMarkers) { "refresh" } else { "append" }
    Invoke-Step "$action AKR section in $Target" {
        if (Test-Path -LiteralPath $Target) {
            Copy-Item -LiteralPath $Target -Destination "$Target.bak" -Force
        }
        if ($hasMarkers) {
            # Singleline so `.` spans the section body; the markers themselves anchor it,
            # so nothing outside the pair is considered.
            $pattern = [regex]::Escape($AgentBegin) + '.*?' + [regex]::Escape($AgentEnd)
            $newText = [regex]::Replace(
                $existing,
                $pattern,
                { param($m) $block },
                [System.Text.RegularExpressions.RegexOptions]::Singleline
            )
        } elseif ([string]::IsNullOrWhiteSpace($existing)) {
            $newText = "$block`n"
        } else {
            $separator = if ($existing.EndsWith("`n")) { "`n" } else { "`n`n" }
            $newText = "$existing$separator$block`n"
        }
        # LF only: these files are read by tools on every platform, and a mixed-ending
        # rewrite of someone else's file is a diff nobody asked for.
        $newText = $newText.Replace("`r`n", "`n")
        [System.IO.File]::WriteAllText($Target, $newText)
    }
    Log "Agent section ${action}ed: $Target"
}

if (-not $NoAgents) {
    Install-AgentSection (Join-Path $HOME ".claude\CLAUDE.md")
    Install-AgentSection (Join-Path $HOME ".codex\AGENTS.md")
    Install-AgentSection (Join-Path $HOME ".config\opencode\AGENTS.md")
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
