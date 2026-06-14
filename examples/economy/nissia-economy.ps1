<#
  nissia-economy.ps1 — token-economy wrapper around the `nissia` browser CLI.

  Why it exists (the 3 classic token leaks an agent has with a browser, and the fix):

    | Leak                                   | Cost            | Fix here                         |
    |----------------------------------------|-----------------|----------------------------------|
    | full-page `snap`                       | 2,000-4,000 tok | always --focus; prefer peek/grab |
    | auto re-snap after every action        | 2-4k per action | `act` uses --no-snap             |
    | base64 screenshots into the context    | huge            | `shot` writes a file, prints path|

  Usage:  pwsh -File nissia-economy.ps1 <verb> [args...]
          (Windows PowerShell 5.1:  powershell -File nissia-economy.ps1 <verb> [args...])

  Verbs:
    open  <url> [selector]      launch-if-needed + snap (focused if you pass a selector)
    snap  [selector]            snap the current page (focused if you pass a selector)
    peek  [selector] [maxlines] read --focus (default selector=main, maxlines=80)  << cheapest READ
    grab  "<js>"                eval JavaScript (compact extraction)               << cheapest DATA
    act   <sub> [args...]       click/fill/type/select/scroll WITHOUT re-snap (--no-snap)  << cheapest ACT
    see   <sub> [args...] <sel> like `act` but re-snaps ONLY <sel> afterwards (--focus)
    shot  [file]                screenshot to a PNG file; prints the path (NO base64 into context)
    up                          launch Chrome (background, idle-timeout 30m, profile "agent")
    down                        stop Chrome
    status                      browser status
    <other>                     passthrough to `nissia` (session, record, replay, agent, search...)

  Config (env vars):
    NISSIA_BIN       path to the nissia binary (default: `nissia` on PATH)
    NISSIA_VISIBLE   set to 1 to launch a VISIBLE Chrome (close your normal Chrome first).
                     Default is headless: starts an ISOLATED instance that always exposes the
                     debug port, even if you already have Chrome open.
#>

[CmdletBinding()]
param(
  [Parameter(Position = 0)][string]$Cmd = 'help',
  [Parameter(Position = 1, ValueFromRemainingArguments = $true)][string[]]$Rest = @()
)

$NISSIA = if ($env:NISSIA_BIN) { $env:NISSIA_BIN } else { 'nissia' }
$ProfileName = 'agent'
$Port = 9222

function Test-Port {
  # Fast TCP check (400ms) so `eval` never hangs when nothing listens on the port.
  try {
    $c = New-Object System.Net.Sockets.TcpClient
    $iar = $c.BeginConnect('127.0.0.1', $Port, $null, $null)
    $ok = $iar.AsyncWaitHandle.WaitOne(400)
    $res = ($ok -and $c.Connected)
    if ($res) { $c.EndConnect($iar) }
    $c.Close()
    return $res
  }
  catch { return $false }
}

function Test-Browser {
  # Functional health-check: port open + eval returns 1. (Do not trust status.running alone.)
  if (-not (Test-Port)) { return $false }
  try {
    $r = & $NISSIA eval "1" 2>$null | Out-String
    return ($r.Trim() -eq '1')
  }
  catch { return $false }
}

function Ensure-Browser {
  if (Test-Browser) { return }
  # Headless by default: an isolated instance always exposes the debug port, even
  # when the user already has a normal Chrome open (a visible launch would hand off
  # to that instance and never open the port). Set NISSIA_VISIBLE=1 to watch live.
  if ($env:NISSIA_VISIBLE -eq '1') {
    & $NISSIA browser launch --background --idle-timeout 30 --profile $ProfileName 2>&1 | Out-Null
  }
  else {
    & $NISSIA browser launch --headless --background --idle-timeout 30 --profile $ProfileName 2>&1 | Out-Null
  }
  for ($i = 0; $i -lt 30; $i++) {
    if (Test-Browser) { return }
    Start-Sleep -Milliseconds 300
  }
}

function Get-Arg([int]$idx, $default = $null) {
  if ($Rest.Count -gt $idx) { return $Rest[$idx] }
  return $default
}

switch ($Cmd.ToLower()) {

  'open' {
    Ensure-Browser
    $url = Get-Arg 0
    $sel = Get-Arg 1
    if ($sel) { & $NISSIA snap $url --focus $sel } else { & $NISSIA snap $url }
  }

  'snap' {
    Ensure-Browser
    $sel = Get-Arg 0
    if ($sel) { & $NISSIA snap --focus $sel } else { & $NISSIA snap }
  }

  'peek' {
    Ensure-Browser
    $sel = Get-Arg 0 'main'
    $lines = Get-Arg 1 '80'
    & $NISSIA read --focus $sel --max-lines $lines
  }

  'grab' {
    Ensure-Browser
    & $NISSIA eval (Get-Arg 0)
  }

  'act' {
    Ensure-Browser
    $sub = Get-Arg 0
    $rargs = @()
    if ($Rest.Count -gt 1) { $rargs = $Rest[1..($Rest.Count - 1)] }
    & $NISSIA $sub @rargs --no-snap
  }

  'see' {
    Ensure-Browser
    $sel = $Rest[$Rest.Count - 1]
    $sub = Get-Arg 0
    $rargs = @()
    if ($Rest.Count -gt 2) { $rargs = $Rest[1..($Rest.Count - 2)] }
    & $NISSIA $sub @rargs --no-snap | Out-Null
    & $NISSIA snap --focus $sel
  }

  'shot' {
    Ensure-Browser
    $file = Get-Arg 0
    if (-not $file) { $file = Join-Path $env:TEMP ("nissia_{0}.png" -f (Get-Date -Format 'HHmmss')) }
    & $NISSIA screenshot --file $file | Out-Null
    Write-Output $file
  }

  'up' { Ensure-Browser; & $NISSIA browser status }
  'down' { & $NISSIA browser stop }
  'status' { & $NISSIA browser status }

  'help' {
    Get-Content $PSCommandPath -TotalCount 45 | Where-Object { $_ -notmatch '^\s*<#|^#>' }
  }

  default { & $NISSIA $Cmd @Rest }
}
