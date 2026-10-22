#!/bin/bash
# docker/observe.sh — installed as /usr/local/sbin/yog-observe, the forced command of the
# observation key: it reads the state of production and changes nothing.
# Under yog-obs it is the gate, and hands the one accepted word to `sudo`; as root, it reads.
# It prints states and numbers only, never a secret or a line someone else wrote.
# Setting it up, and the rule it keeps: docker/README.md, "The observation key".
set -euo pipefail
# ⚠️ The strict patterns hold only in the C locale: under UTF-8, `[A-Za-z]` also matches `É`.
export LC_ALL=C

readonly installed=/usr/local/sbin/yog-observe
readonly checkout=/srv/yog-sothoth

refuse() {
  echo "yog-observe: refused: '$1' (accepted: health, metrics, db)" >&2
  exit 2
}

# ⚠️ Matched whole and anchored, never split: `$` ends the string, so a newline cannot smuggle a
# second command past it.
accepted() { [[ $1 =~ ^(health|metrics|db)$ ]]; }

if [ "$EUID" -ne 0 ]; then
  word=${SSH_ORIGINAL_COMMAND:-}
  accepted "$word" || refuse "$word"
  exec sudo -n "$installed" "$word"
fi

if [ $# -ne 1 ] || ! accepted "$1"; then
  refuse "$*"
fi

cd "$checkout"
exec 9>/run/yog-observe.lock
flock -n 9 || { echo "yog-observe: another reading is running" >&2; exit 3; }

# ⚠️ A copy of update.sh's list: sourcing it would make root run whatever the checkout holds.
readonly services=(postgres yog-indexer yog-context yog-signals yog-api yog-archive yog-analytic yog-web)
readonly daemons=(yog-indexer yog-context yog-signals yog-analytic)

bounded() { timeout 60 "$@"; }
dc() { bounded docker compose -f docker-compose.yml -f docker-compose.prod.yml --profile full "$@"; }

# Every line printed matches one of these; the others are only counted.
readonly metric_line='^yog_[a-z0-9_]+(\{[a-z_]+="[A-Za-z0-9_.:+-]*"(,[a-z_]+="[A-Za-z0-9_.:+-]*")*\})? (-?[0-9.e+-]+|NaN|[+-]Inf)$'
readonly db_line='^[a-z0-9_]+ [a-z0-9_]+$'

strict() { # <regex>: stdin's matching lines, then how many others were left out
  local line refused=0
  while IFS= read -r line; do
    if [[ $line =~ $1 ]]; then
      printf '%s\n' "$line"
    elif [[ $line != '# HELP '* && $line != '# TYPE '* && -n $line ]]; then
      refused=$((refused + 1))
    fi
  done
  echo "refused_lines $refused"
}

health() {
  local s id warn error
  for s in "${services[@]}"; do
    id=$(dc ps -a -q "$s" 2>/dev/null) || id=""
    if [ -z "$id" ]; then
      echo "service $s missing"
      continue
    fi
    echo "service $s $(bounded docker inspect -f \
      '{{.State.Status}} started={{.State.StartedAt}} restarts={{.RestartCount}} oom_killed={{.State.OOMKilled}}' "$id")"
    # Counted, never printed: a log line can quote a token name anyone may publish.
    warn=$(dc logs --no-color --since 1h "$s" 2>&1 | grep -c -E '\bWARN(ING)?\b') || true
    error=$(dc logs --no-color --since 1h "$s" 2>&1 | grep -c -E '\b(ERROR|FATAL|PANIC)\b') || true
    echo "logs_last_hour $s warn=$warn error=$error"
  done

  local ids
  ids=$(dc ps -q)
  # ⚠️ Never without ids: a bare `docker stats` lists every container on the host, other projects'.
  if [ -n "$ids" ]; then
    # shellcheck disable=SC2086 # one argument per container id
    bounded docker stats --no-stream --format 'memory {{.Name}} {{.MemUsage}} cpu={{.CPUPerc}}' $ids
  fi
  df -P / | awk 'NR == 2 { print "disk / used=" $3 "k available=" $4 "k " $5 }'
  free -m | awk '$1 == "Mem:" { print "host_memory_mib total=" $2 " used=" $3 " available=" $7 } $1 == "Swap:" { print "host_swap_mib total=" $2 " used=" $3 }'

  local mine theirs
  mine=$(sha256sum <"$installed" | cut -c1-12)
  theirs=$(sha256sum <"$checkout/docker/observe.sh" | cut -c1-12)
  echo "observe installed=$mine checkout=$theirs $([ "$mine" = "$theirs" ] && echo match || echo differs)"
}

# Through yog-web's busybox wget, as update.sh reaches /readyz: the image of the daemons has no
# HTTP client, and production publishes no metrics port on the host.
metrics() {
  local s body
  for s in "${daemons[@]}"; do
    echo "== $s"
    if ! body=$(dc exec -T yog-web wget -q -O - -T 10 "http://$s:9000/metrics" 2>/dev/null); then
      echo "unreachable"
      continue
    fi
    printf '%s\n' "$body" | strict "$metric_line"
  done
}

# As yog_api, which may write nothing, inside the postgres container, where local connections are
# trusted: no secret is read. Read-only and bounded on top, whatever the role may do.
db() {
  # shellcheck disable=SC2016 # $POSTGRES_DB is the container's
  dc exec -T -e PGOPTIONS='-c default_transaction_read_only=on -c statement_timeout=15s' postgres \
    sh -c 'exec psql -X -q -At -F " " -v ON_ERROR_STOP=1 -U yog_api -d "$POSTGRES_DB"' <<'SQL' | strict "$db_line"
SELECT 'network_slot', slot FROM network_status;
SELECT 'network_rpc_latency_ms', rpc_latency_ms FROM network_status;
SELECT 'network_observed_age_s', extract(epoch FROM now() - observed_at)::bigint FROM network_status;
SELECT 'last_swap_age_s', coalesce(extract(epoch FROM now() - max(timestamp))::bigint::text, 'none_in_24h')
  FROM swap_events WHERE timestamp > now() - interval '1 day';
SELECT 'swaps_last_hour', count(*) FROM swap_events WHERE timestamp > now() - interval '1 hour';
SELECT 'pools', count(*) FROM pools;
SELECT 'pools_needs_refresh', count(*) FROM pools WHERE needs_refresh;
SELECT 'signals_last_24h', count(*) FROM signals WHERE triggered_at > now() - interval '1 day';
SELECT 'database_bytes', pg_database_size(current_database());
SQL
}

"$1"
