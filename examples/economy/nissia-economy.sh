#!/usr/bin/env bash
# nissia-economy.sh — token-economy wrapper around the `nissia` browser CLI.
#
# The 3 classic token leaks an agent has with a browser, and the fix:
#   | Leak                                | Cost            | Fix                              |
#   | full-page snap                      | 2,000-4,000 tok | always --focus; prefer peek/grab |
#   | auto re-snap after every action     | 2-4k per action | `act` uses --no-snap             |
#   | base64 screenshots into context     | huge            | `shot` writes a file, prints path|
#
# Usage:  ./nissia-economy.sh <verb> [args...]
#
# Verbs:
#   open <url> [sel]      launch-if-needed + snap (focused if you pass a selector)
#   snap [sel]            snap current page (focused if sel given)
#   peek [sel] [n]        read --focus (default sel=main, n=80)   << cheapest READ
#   grab "<js>"           eval JavaScript                          << cheapest DATA
#   act <sub> [args...]   click/fill/type/select/scroll WITHOUT re-snap (--no-snap)
#   see <sub> [args] <sel> like act, then re-snap ONLY <sel>
#   shot [file]           screenshot to file; prints path (no base64 into context)
#   up | down | status    browser lifecycle
#   <other>               passthrough to nissia (session, record, replay, agent, search...)
#
# Env:  NISSIA_BIN (default: nissia)   NISSIA_VISIBLE=1 (visible Chrome; default headless)

set -uo pipefail
NISSIA="${NISSIA_BIN:-nissia}"
PROFILE="agent"
PORT=9222

port_open() { (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null && { exec 3>&- 3<&-; return 0; } || return 1; }
browser_up() { port_open || return 1; [ "$("$NISSIA" eval 1 2>/dev/null | tr -d '[:space:]')" = "1" ]; }
ensure() {
  browser_up && return 0
  # Headless by default: an isolated instance always exposes the debug port even
  # when a normal Chrome is already open. NISSIA_VISIBLE=1 to watch live.
  if [ "${NISSIA_VISIBLE:-}" = "1" ]; then
    "$NISSIA" browser launch --background --idle-timeout 30 --profile "$PROFILE" >/dev/null 2>&1 || true
  else
    "$NISSIA" browser launch --headless --background --idle-timeout 30 --profile "$PROFILE" >/dev/null 2>&1 || true
  fi
  for _ in $(seq 1 30); do browser_up && return 0; sleep 0.3; done
}

cmd="${1:-help}"; shift || true
case "$cmd" in
  open)   ensure; url="${1:-}"; sel="${2:-}"
          if [ -n "$sel" ]; then "$NISSIA" snap "$url" --focus "$sel"; else "$NISSIA" snap "$url"; fi ;;
  snap)   ensure; sel="${1:-}"
          if [ -n "$sel" ]; then "$NISSIA" snap --focus "$sel"; else "$NISSIA" snap; fi ;;
  peek)   ensure; "$NISSIA" read --focus "${1:-main}" --max-lines "${2:-80}" ;;
  grab)   ensure; "$NISSIA" eval "${1:-}" ;;
  act)    ensure; sub="${1:-}"; shift || true; "$NISSIA" "$sub" "$@" --no-snap ;;
  see)    ensure; sub="${1:-}"; shift || true
          sel="${*: -1}"; set -- "${@:1:$(($#-1))}"
          "$NISSIA" "$sub" "$@" --no-snap >/dev/null; "$NISSIA" snap --focus "$sel" ;;
  shot)   ensure; f="${1:-/tmp/nissia_$(date +%H%M%S).png}"; "$NISSIA" screenshot --file "$f" >/dev/null; echo "$f" ;;
  up)     ensure; "$NISSIA" browser status ;;
  down)   "$NISSIA" browser stop ;;
  status) "$NISSIA" browser status ;;
  help)   sed -n '2,38p' "$0" ;;
  *)      "$NISSIA" "$cmd" "$@" ;;
esac
