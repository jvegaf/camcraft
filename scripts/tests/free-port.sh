#!/usr/bin/env bash
# Behaviour tests for `scripts/free-port.sh`: a free port is cheap, a previous Vite listener is
# stopped, and a listener that is not Vite is never killed. The shared fixture handles temp state;
# each case picks a fresh ephemeral port so the suite can run next to a real dev server.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"

SCRIPT="$REPO_ROOT/scripts/free-port.sh"
PIDS=""

cleanup() {
  local pid
  for pid in $PIDS; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  for pid in $PIDS; do
    wait "$pid" 2>/dev/null || true
  done
}
trap 'cleanup; rm -rf "$WORK"' EXIT

free_port() {
  python3 - <<'PY'
import socket
sock = socket.socket()
sock.bind(("127.0.0.1", 0))
print(sock.getsockname()[1])
sock.close()
PY
}

listening() {
  local port=$1
  (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null
}

wait_for_listen() {
  local port=$1 i
  for i in $(seq 1 100); do
    if listening "$port"; then return 0; fi
    sleep 0.05
  done
  return 1
}

echo "== a free port stays free =="
port=$(free_port)
expect_ok "un puerto libre se informa libre" "$SCRIPT" "$port"
expect_contains "el mensaje nombra el puerto libre" "$WORK/out" "$port"
expect_contains "el mensaje dice que está libre" "$WORK/out" "libre"

echo "== a previous Vite listener is stopped =="
port=$(free_port)
bash -c 'exec -a "node /x/node_modules/vite/bin/vite.js" python3 -m http.server "$1" --bind 127.0.0.1' _ "$port" \
  >/dev/null 2>&1 &
vite_pid=$!
PIDS="$PIDS $vite_pid"
if wait_for_listen "$port"; then
  pass "el Vite de prueba escucha en $port"
else
  fail "el Vite de prueba no llegó a escuchar en $port"
fi
started=$SECONDS
expect_ok "un Vite anterior se detiene" "$SCRIPT" "$port"
elapsed=$((SECONDS - started))
expect_contains "el mensaje nombra el pid detenido" "$WORK/out" "$vite_pid"
if kill -0 "$vite_pid" 2>/dev/null; then
  fail "el Vite anterior sigue vivo ($vite_pid)"
else
  pass "el Vite anterior ya no existe"
fi
if listening "$port"; then
  fail "el puerto $port sigue ocupado tras detener Vite"
else
  pass "el puerto $port quedó libre"
fi
if [ "$elapsed" -le 10 ]; then
  pass "la espera no se colgó (${elapsed}s)"
else
  fail "la espera tardó demasiado (${elapsed}s)"
fi

echo "== a listener that is not Vite survives =="
port=$(free_port)
python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1 &
other_pid=$!
PIDS="$PIDS $other_pid"
if wait_for_listen "$port"; then
  pass "el proceso no-Vite escucha en $port"
else
  fail "el proceso no-Vite no llegó a escuchar en $port"
fi
expect_fail "un proceso que no es Vite no se mata" "$SCRIPT" "$port"
expect_contains "el error nombra al ocupante" "$WORK/err" "$other_pid"
expect_contains "el error pide liberarlo a mano" "$WORK/err" "mano"
if kill -0 "$other_pid" 2>/dev/null; then
  pass "el proceso no-Vite sigue vivo"
else
  fail "el proceso no-Vite fue matado"
fi
if listening "$port"; then
  pass "el proceso no-Vite sigue escuchando"
else
  fail "el proceso no-Vite dejó de escuchar"
fi

summary
