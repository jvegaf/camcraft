#!/usr/bin/env bash
# Libera el puerto del servidor de desarrollo: detiene un Vite anterior si lo hay y se niega a tocar
# cualquier otro proceso que esté escuchando. Así `strictPort` puede relanzar sin fallar.
set -euo pipefail

PORT=${1:-1420}
if ! [[ "$PORT" =~ ^[0-9]+$ ]]; then
  echo "ERROR: puerto inválido: $PORT" >&2
  exit 2
fi

# ¿Hay algo escuchando en el puerto? Se responde sin depender de que el proceso sea nuestro.
have_listener() {
  if command -v ss >/dev/null 2>&1; then
    [ -n "$(ss -ltnH "sport = :$PORT" 2>/dev/null || true)" ]
  elif command -v lsof >/dev/null 2>&1; then
    [ -n "$(lsof -ti "tcp:$PORT" 2>/dev/null || true)" ]
  else
    echo "ERROR: no hay «ss» ni «lsof» para consultar el puerto $PORT" >&2
    return 2
  fi
}

# PIDs que escuchan en el puerto. `ss` es la vía normal; `lsof` solo si no está disponible.
listener_pids() {
  if command -v ss >/dev/null 2>&1; then
    ss -ltnpH "sport = :$PORT" 2>/dev/null | grep -oE 'pid=[0-9]+' | cut -d= -f2 | sort -u || true
  elif command -v lsof >/dev/null 2>&1; then
    lsof -ti "tcp:$PORT" 2>/dev/null || true
  fi
}

if have_listener; then
  :
else
  status=$?
  if [ "$status" -eq 2 ]; then
    exit 1
  fi
  echo "Puerto $PORT libre."
  exit 0
fi

pids=$(listener_pids)
if [ -z "$pids" ]; then
  echo "ERROR: el puerto $PORT está ocupado pero no se pudo identificar el proceso." >&2
  echo "Libéralo a mano antes de lanzar el servidor." >&2
  exit 1
fi

# Clasifica todo antes de matar nada: si hay un ocupante ajeno, no se detiene ni un Vite.
vite_pids=""
for pid in $pids; do
  args=$(ps -p "$pid" -o args= 2>/dev/null || true)
  if [ -z "$args" ]; then
    continue
  fi
  if [[ "$args" == *vite* ]]; then
    vite_pids="$vite_pids $pid"
  else
    echo "ERROR: el puerto $PORT lo ocupa un proceso que no es Vite: pid $pid ($args)" >&2
    echo "Libéralo a mano (por ejemplo: kill $pid) antes de lanzar el servidor." >&2
    exit 1
  fi
done

if [ -z "$vite_pids" ]; then
  echo "ERROR: el puerto $PORT está ocupado pero ningún proceso identificado es Vite." >&2
  echo "Libéralo a mano antes de lanzar el servidor." >&2
  exit 1
fi

for pid in $vite_pids; do
  args=$(ps -p "$pid" -o args= 2>/dev/null || true)
  echo "Deteniendo el servidor de desarrollo anterior: pid $pid ($args)"
  kill -TERM "$pid" 2>/dev/null || true
done

# Espera activa: hasta 5 s a que el puerto se libere.
deadline=$((SECONDS + 5))
while have_listener; do
  if [ "$SECONDS" -ge "$deadline" ]; then
    survivors=""
    for pid in $vite_pids; do
      if kill -0 "$pid" 2>/dev/null; then
        survivors="$survivors $pid"
      fi
    done
    echo "ERROR: el puerto $PORT sigue ocupado tras 5 s; pids supervivientes:${survivors:- desconocidos}" >&2
    exit 1
  fi
  sleep 0.1
done

echo "Puerto $PORT libre."
