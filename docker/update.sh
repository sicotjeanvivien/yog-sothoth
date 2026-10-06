#!/usr/bin/env bash
# docker/update.sh — update a running production server to a version.
#
#   docker/update.sh vX.Y.Z           deploy that version, unless it is the one deployed
#   docker/update.sh --force vX.Y.Z   run the whole sequence even if it is
#   docker/update.sh --check          run the post-deploy checks only, change nothing
#
# A version is a tag on main; docker/README.md says when to cut one. Nothing
# else is deployed: not main as it stands, not a branch, not a bare commit.
#
# Run it on the server, from anywhere. It is the one definition of the update
# procedure; a deployment pipeline calls this script and nothing else.
#
# It detaches itself. The work runs in its own session, writing to a log
# under .git/yog-update/logs/, and the foreground only follows that log and
# returns its exit code. An SSH session that drops, or a Ctrl-C, ends the
# foreground and leaves the update running: stopping it half-way would leave
# the daemons down. Re-attach with `tail -f` on the log it names; abort with
# `kill -- -<pid>` on the pid it names (the whole session, docker included).
# The follower needs GNU tail (--pid) and util-linux setsid (-w).
#
# The sequence, and why each step sits where it does
# --------------------------------------------------
# 1. Refuse to start unless: no other run holds the lock, no tracked file is
#    modified (the .env is ignored by git), HEAD is an ancestor of
#    origin/main (no local commit on the server), and the version is a tag
#    that origin holds on the same commit, on main's first-parent line — what
#    goes to production is what main went through, not a commit of a merged
#    branch. Two refusals guard the way back: a version older than versioned
#    deploys (its scripts would put the old gate back), and a version that
#    does not ship every migration the database holds (read from
#    _sqlx_migrations) — the schema cannot go back with the code.
# 2. Check the version out, detached — the checkout IS the version deployed —
#    then hand over to the script as it is in that commit: a deploy always
#    runs the procedure it ships with.
# 3. Pull the registry images and build ours while the previous version
#    still serves: the outage lasts only for steps 4-6.
# 4. Stop every daemon that talks to the database. `up -d` alone is not
#    enough: it runs yog-migrate before STARTING services, but services
#    already running keep running through the migration, the old code
#    against the new schema. A migration that raises `pools.needs_refresh`
#    for a back-fill then has its flags consumed by the old yog-context,
#    which lowers them without writing what it does not know about: the
#    feature stays empty, everything looks green.
# 5. Migrate (`compose run yog-migrate`, i.e. `bootstrap`, idempotent). The
#    pools flagged `needs_refresh` are read just before and just after, with
#    every daemon stopped: the difference is what the migration flagged.
# 6. `up -d`, and record the commit and the version as deployed
#    (.git/yog-update/deployed, deployed-version). What is deployed is that
#    record, not the checkout: a run that fails after the checkout leaves the
#    record behind it, and the next run of the same version deploys again.
# 7. Checks: every service running and stable (no restart for a minute),
#    yog-api's /readyz (it pings the database) and the dashboard answering
#    from inside the compose network, and the pools flagged by the migration
#    drained by yog-context. A pool yog-context cannot resolve (closed on
#    chain, rejected account) keeps its flag for good: once the count stops
#    falling, what remains is reported as a warning. The check fails on a
#    count that never moves, on a plateau of 100 or more (yog-context takes
#    the 100 oldest per tick, so they starve the rest), and on a count still
#    falling at the deadline.
#
# If the migration fails, the script stops with the daemons DOWN. The images
# already hold the new code, which must not run against the old schema. The
# message says whether the previous version can still be served.
#
# What this script does not do
# ----------------------------
# - Roll a schema back. Migrations are forward-only; going back is a restore
#   (crates/persistence/README.md, "Backup and restore"). Which is why a
#   version that does not ship every applied migration is refused: fix
#   forward, with a new version.
# - Refresh a continuous aggregate. A full-range refresh typed by hand once
#   retention has dropped a chunk erases materialised history:
#   crates/persistence/migrations/README.md says which bounded form to use.
# - Check the drain from --check: it needs the flags read around a migration.
#
# Trap: the checkout rewrites this very file while bash is running it,
# and bash reads a script as it goes. Everything therefore lives in `main`,
# parsed whole before it runs, and the last line calls it and exits at once.
#
# Tunables (environment): UPDATE_STABLE_SECS (default 60),
# UPDATE_DRAIN_TIMEOUT_SECS (default 600). yog-context resolves 100 pools per
# protocol every CONTEXT_METADATA_POLL_SECS (10 s by default).

main() {
  set -euo pipefail
  local -r script=$(readlink -f "$0")
  cd "$(dirname "$script")/.."

  dc=(docker compose -f docker-compose.yml -f docker-compose.prod.yml --profile full)
  db_daemons=(yog-indexer yog-context yog-signals yog-api yog-archive)
  services=(postgres "${db_daemons[@]}" yog-web caddy)
  state=$(git rev-parse --absolute-git-dir)/yog-update
  mkdir -p "$state/logs"
  local -r stable_secs="${UPDATE_STABLE_SECS:-60}"
  local -r drain_timeout_secs="${UPDATE_DRAIN_TIMEOUT_SECS:-600}"

  local mode version=""
  case "${1:-}" in
    --check) [ $# -eq 1 ] || usage; mode=check ;;
    --force) [ $# -eq 2 ] || usage; mode=force; version=$2 ;;
    v*) [ $# -eq 1 ] || usage; mode=deploy; version=$1 ;;
    *) usage ;;
  esac
  # The same pattern as docker/deploy-entry.sh, which the deployment key goes
  # through first; this one guards a call typed on the server.
  [ "$mode" = check ] || [[ $version =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || usage

  if [ "$mode" = check ]; then
    exec 9>"$state/lock"
    if ! flock -n 9; then
      # The only window a pipeline has on a detached update: the log itself
      # is not reachable through the deployment key.
      # The run holding the lock names its log in $state/current; a log
      # whose exit code is already written is over, and then the lock is
      # held by another --check.
      local current
      current=$(cat "$state/current" 2>/dev/null || true)
      if [ -n "$current" ] && [ -f "$current" ] && [ ! -s "$current.rc" ]; then
        log "an update is running; the end of its log ($current):"
        tail -n 40 "$current"
        die "an update is running: check again once it is over"
      fi
      die "another --check is running"
    fi
    log "deployed: $(deployed_label)"
    checks "$stable_secs" || die "checks failed (see above)"
    log "✅ all checks passed"
    return
  fi

  if [ -z "${YOG_UPDATE_RC:-}" ]; then
    follow_detached "$script" "$@"
  fi
  # Under set -e a failing command in an EXIT trap replaces the exit code:
  # if the code cannot be written, the follower says so instead.
  trap 'echo $? >"$YOG_UPDATE_RC" || true' EXIT

  local old new
  if [ -z "${YOG_UPDATE_FROM:-}" ]; then
    # 1. Preconditions
    exec 9>"$state/lock"
    flock -n 9 || die "another update is running (lock $state/lock)"
    echo "${YOG_UPDATE_RC%.rc}" >"$state/current"
    [ -z "$(git status --porcelain --untracked-files=no)" ] \
      || die "tracked files are modified: $(git status --porcelain --untracked-files=no | tr '\n' ' ')"
    # --tags: a tag moved after the fact is refused by git rather than
    # followed — a version names one commit, for good. --quiet silences that
    # refusal too: without the die, the deploy would stop on an empty log.
    git fetch --quiet --tags origin main \
      || die "fetching origin failed: unreachable, or a tag moved there (\`git fetch --tags origin\` says which)"
    git merge-base --is-ancestor HEAD origin/main \
      || die "HEAD is not an ancestor of origin/main: local commits on the server?"

    # The tag as origin holds it now, not as the server remembers it: a tag
    # deleted there is not deployed, and one that names another commit there
    # is refused rather than followed. The last line is the peeled commit of
    # an annotated tag, the only line of a lightweight one.
    local remote
    remote=$(git ls-remote origin "refs/tags/$version" "refs/tags/$version^{}" | awk 'END { print $1 }') \
      || die "cannot list the tags of origin"
    [ -n "$remote" ] || die "no tag $version on origin"
    new=$(git rev-parse --verify --quiet "refs/tags/$version^{commit}") \
      || die "tag $version is on origin but was not fetched"
    [ "$new" = "$remote" ] \
      || die "tag $version names ${remote:0:7} on origin and ${new:0:7} here: a version names one commit for good"
    # First-parent, not merely an ancestor: the intermediate commits of a
    # merged pull request are ancestors of main too, and a version tagged on
    # one would miss whatever was merged beside it. Not `grep -q`: it stops
    # reading at the match, rev-list takes a SIGPIPE, and pipefail reads that
    # as "not on main".
    # shellcheck disable=SC2143
    [ -n "$(git rev-list --first-parent origin/main | grep -x "$new")" ] \
      || die "$version is not on main's first-parent line: a version is a commit main went through, not one of a merged branch"
    [[ $(git show "$new:docker/update.sh" 2>/dev/null) == *deployed-version* ]] \
      || die "$version predates versioned deploys: its scripts would put the old gate back — deploy a later version"

    # The migrations the database holds and this version does not ship. The
    # database, not git: a failed deploy may have applied part of its own,
    # a checkout may have been moved by hand, a file may have been renamed —
    # what the older binaries refuse is a schema newer than theirs, and only
    # _sqlx_migrations says what the schema is.
    local applied shipped unknown
    applied=$(psql_admin -c "SELECT version FROM _sqlx_migrations ORDER BY 1") \
      || die "cannot read the applied migrations: is postgres up?"
    shipped=$(git ls-tree --name-only "$new" crates/persistence/migrations/ \
      | sed -n 's#^.*/0*\([0-9][0-9]*\)_[^/]*\.sql$#\1#p')
    unknown=$(LC_ALL=C comm -23 <(LC_ALL=C sort <<<"$applied") <(LC_ALL=C sort <<<"$shipped") | tr '\n' ' ')
    [ -z "${unknown// /}" ] \
      || die "$version does not ship migration(s) ${unknown% } that the database holds: the older binaries refuse a schema newer than theirs — fix forward, with a new version"

    # No record means unknown, not "HEAD is deployed": a checkout pulled by
    # hand, or a first run that failed after its checkout, deploys. And the
    # record alone is not enough either: a run that failed after its checkout
    # left HEAD elsewhere, with the daemons possibly stopped.
    old=$(cat "$state/deployed" 2>/dev/null || echo unknown)
    if [ "$old" = "$new" ] && [ "$(git rev-parse HEAD)" = "$new" ] && [ "$mode" = deploy ]; then
      local recorded
      recorded=$(cat "$state/deployed-version" 2>/dev/null || echo "no version")
      if [ "$recorded" != "$version" ]; then
        echo "$version" >"$state/deployed-version"
        log "nothing to deploy: $version names the commit deployed as $recorded (${new:0:7}); recorded as $version"
      else
        log "nothing to deploy: $version (${new:0:7}) is deployed"
      fi
      return
    fi
    if [ "$old" != unknown ] && [ "$old" != "$new" ] && git merge-base --is-ancestor "$new" "$old"; then
      log "going back to $version: the database holds no migration it lacks"
    fi

    # 2. Check the version out, then run the procedure as it has it. The lock
    #    (fd 9) and the exit-code file go along through the exec.
    log "deploying $(deployed_label) → $version (${new:0:7})"
    [ "$old" = unknown ] || git log --oneline "$old..$new" 2>/dev/null || true
    git checkout --quiet --detach "$new"
    YOG_UPDATE_FROM=$old exec "$script" "$@"
  fi
  old=$YOG_UPDATE_FROM
  new=$(git rev-parse HEAD)
  local -r from=$(deployed_label)

  # 3. Build while the previous version still serves
  log "pulling registry images, building ours"
  "${dc[@]}" pull --quiet --ignore-buildable
  "${dc[@]}" build
  local migrations_before migrations_after
  migrations_before=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations")

  # 4. Stop the daemons that talk to the database
  local -r outage_start=$(date +%s)
  log "stopping ${db_daemons[*]}"
  "${dc[@]}" stop "${db_daemons[@]}"

  # 5. Migrate, reading the flags on each side of it
  flagged_pools >"$state/flagged.before"
  log "migrating"
  if ! "${dc[@]}" run --rm -T yog-migrate; then
    migrations_after=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations" 2>/dev/null) || migrations_after=unknown
    log "❌ the migration failed. The daemons are STOPPED, on purpose: the images" >&2
    log "   now hold the new code, which must not run against the old schema." >&2
    if [ "$migrations_after" = unknown ]; then
      log "   The database does not answer either: whether a migration of this" >&2
      log "   deploy was applied is unknown. Bring Postgres back, then compare" >&2
      log "   _sqlx_migrations (${migrations_before} rows before) with the migrations directory." >&2
    elif [ "$migrations_after" = "$migrations_before" ]; then
      log "   No migration of this deploy was applied: the previous version can" >&2
      log "   still be served — git checkout --detach ${old:0:7}, then" >&2
      log "   ${dc[*]} build && ${dc[*]} up -d" >&2
      log "   and fix forward on main before the next run, which would bring it back." >&2
    else
      log "   $(( migrations_after - migrations_before )) migration(s) of this deploy were applied before the" >&2
      log "   failure: the previous binaries refuse a schema newer than theirs." >&2
      log "   Fix forward (a new migration, never an edited one), or restore." >&2
    fi
    exit 1
  fi
  migrations_after=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations")
  flagged_pools | LC_ALL=C sort | LC_ALL=C comm -13 <(LC_ALL=C sort "$state/flagged.before") - \
    >"$state/flagged.by-migration"

  # 6. Start, and record what is now deployed
  # --no-deps with the services named: yog-migrate has just run, and `up`
  # would otherwise run it a second time inside the outage.
  log "starting"
  "${dc[@]}" up -d --no-deps "${services[@]}"
  echo "$new" >"$state/deployed"
  echo "$version" >"$state/deployed-version"
  local -r outage_secs=$(( $(date +%s) - outage_start ))

  # 7. Checks
  local failed=0
  checks "$stable_secs" || failed=1
  drain "$state/flagged.by-migration" "$drain_timeout_secs" || failed=1

  log "summary: $from → $version (${new:0:7}), $(( migrations_after - migrations_before )) migration(s) applied, outage ${outage_secs}s"
  [ "$failed" -eq 0 ] || die "deployed, but a check failed (see above)"
  log "✅ deployed and checked"
}

log() { printf '%s  %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }
die() { log "❌ $*" >&2; exit 1; }
usage() { echo "usage: $0 vX.Y.Z | --force vX.Y.Z | --check" >&2; exit 2; }

# What the record says is deployed: `v0.2.0 (1a2b3c4)`. A record from before
# versions has no tag beside its commit, and nothing at all is `unknown`.
deployed_label() {
  local commit tag
  commit=$(cat "$state/deployed" 2>/dev/null) || { echo unknown; return; }
  tag=$(cat "$state/deployed-version" 2>/dev/null) || tag="no version"
  echo "$tag (${commit:0:7})"
}

# Start the update in its own session and follow its log until it writes its
# exit code. Never returns.
follow_detached() { # <script> <args...>
  local -r script=$1; shift
  local -r logf="$state/logs/$(date -u +%Y%m%dT%H%M%SZ)-$$.log"
  local -r rcf="$logf.rc"
  local tail_pid=""
  # Traps first: a Ctrl-C between the two `&` below and their installation
  # would leave tail, which ignores SIGINT as a background job of a
  # non-interactive shell, printing over the next prompt. The EXIT trap is the
  # one place that stops it; under set -e each of its commands must tolerate
  # failure, or it replaces the exit code (a killed tail makes `wait` 143).
  trap '[ -z "$tail_pid" ] || { kill "$tail_pid" 2>/dev/null || true; wait "$tail_pid" 2>/dev/null || true; }' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  trap 'exit 129' HUP
  : >"$logf"
  # A background job of a non-interactive shell is not a process-group
  # leader, so setsid execs the script in place: $! is the run itself, the
  # leader of its new session. (-w only matters if setsid had to fork; it
  # then waits, so $! still lives exactly as long as the run.)
  YOG_UPDATE_RC=$rcf setsid -w "$script" "$@" >"$logf" 2>&1 </dev/null &
  local -r run_pid=$!
  echo "update running detached as pid $run_pid; log: $logf"
  echo "  re-attach: tail -f $logf    abort: kill -- -$run_pid  (its whole session)"
  # tail stops by itself once the run has exited, after printing the rest of
  # the log: nothing to kill on the normal path, and no last line lost.
  tail -n +1 -f --pid="$run_pid" "$logf" &
  tail_pid=$!
  wait "$tail_pid" || true
  tail_pid=""   # reaped: its pid may already belong to another process
  if [ ! -s "$rcf" ]; then
    # tail can end before the run: a closed pipe when the session drops, a
    # kill. Say which case this is rather than declare the run over.
    kill -0 "$run_pid" 2>/dev/null \
      && die "the follower lost the log, but the update (pid $run_pid) is still running: tail -f $logf"
    die "the update (pid $run_pid) ended without an exit code: read $logf"
  fi
  exit "$(cat "$rcf")"
}

# psql as the admin role, with the credentials of the postgres container itself.
# The single quotes are the point: $POSTGRES_USER is the container's, not ours.
psql_admin() {
  # shellcheck disable=SC2016
  "${dc[@]}" exec -T postgres sh -c \
    'exec psql -X -q -At -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"' sh "$@"
}

flagged_pools() { psql_admin -c "SELECT pool_address FROM pools WHERE needs_refresh ORDER BY 1"; }

container_field() { # <service> <go-template>
  local id
  id=$("${dc[@]}" ps -a -q "$1")
  [ -n "$id" ] || { echo missing; return; }
  docker inspect -f "$2" "$id"
}

checks() { # <stable_secs>
  local failed=0 s
  local -A restarts=()
  for s in "${services[@]}"; do
    local status
    status=$(container_field "$s" '{{.State.Status}}')
    if [ "$status" = running ]; then
      restarts[$s]=$(container_field "$s" '{{.RestartCount}}')
    else
      log "❌ $s is $status, not running"; failed=1
    fi
  done

  # From inside the compose network, through caddy's busybox wget (which
  # fails on any non-2xx). A service just started may not listen yet: keep
  # trying for 60 s before calling it a failure.
  local api="" web="" body="" deadline=$(( $(date +%s) + 60 ))
  while :; do
    if [ "$api" != ok ]; then
      body=$("${dc[@]}" exec -T caddy wget -q -O - -T 5 http://yog-api:5000/readyz 2>&1) || body=""
      [[ $body == *'"ready"'* ]] && api=ok
    fi
    [ "$web" = ok ] || web=$("${dc[@]}" exec -T caddy wget -q -O /dev/null -T 5 http://yog-web:3000/ 2>&1 && echo ok || true)
    if { [ "$api" = ok ] && [ "$web" = ok ]; } || [ "$(date +%s)" -ge "$deadline" ]; then break; fi
    sleep 3
  done
  [ "$api" = ok ] || { log "❌ yog-api /readyz did not answer ready within 60 s"; failed=1; }
  [ "$web" = ok ] || { log "❌ yog-web did not answer 2xx within 60 s"; failed=1; }

  log "watching restarts for ${1}s"
  sleep "$1"
  for s in "${!restarts[@]}"; do
    local now status
    status=$(container_field "$s" '{{.State.Status}}')
    now=$(container_field "$s" '{{.RestartCount}}')
    if [ "$status" != running ] || [ "$now" != "${restarts[$s]}" ]; then
      log "❌ $s is unstable: $status, restarts ${restarts[$s]} → $now"; failed=1
    fi
  done
  [ "$failed" -eq 0 ] && log "services running and stable, /readyz ready, dashboard ok"
  return "$failed"
}

# How many of the pools in <file> are still flagged. The list goes in on
# stdin (COPY), never as an argument: a back-fill can flag thousands of pools.
still_flagged() { # <file>
  { echo "CREATE TEMP TABLE flagged (pool_address text); COPY flagged FROM STDIN;"
    cat "$1"
    printf '%s\n' '\.' "SELECT count(*) FROM pools JOIN flagged USING (pool_address) WHERE needs_refresh;"
  } | psql_admin
}

drain() { # <file of pools flagged by the migration> <timeout_secs>
  local total remaining last="" flat=0 deadline
  total=$(grep -c . "$1" || true)
  if [ "$total" -eq 0 ]; then
    log "the migration flagged no pool needs_refresh: nothing to drain"
    return 0
  fi
  deadline=$(( $(date +%s) + $2 ))
  while :; do
    remaining=$(still_flagged "$1")
    [[ $remaining =~ ^[0-9]+$ ]] || { log "❌ could not count the flagged pools: ${remaining:-nothing}"; return 1; }
    log "needs_refresh: ${remaining}/${total} of the pools the migration flagged still pending"
    [ "$remaining" -eq 0 ] && return 0
    if [ "$remaining" = "$last" ]; then flat=$(( flat + 1 )); else flat=0; fi
    last=$remaining
    if [ "$flat" -lt 4 ] && [ "$(date +%s)" -ge "$deadline" ]; then
      log "❌ still draining at the deadline (${remaining} left, still falling): raise UPDATE_DRAIN_TIMEOUT_SECS,"
      log "   or follow it: SELECT count(*) FROM pools WHERE needs_refresh"
      return 1
    fi
    if [ "$flat" -ge 4 ]; then
      if [ "$remaining" -eq "$total" ]; then
        log "❌ none of the ${total} pools flagged by the migration was resolved: is yog-context working?"
        return 1
      fi
      # yog-context takes the 100 oldest flagged pools per protocol each
      # tick: 100 that never resolve hold back every pool behind them.
      if [ "$remaining" -ge 100 ]; then
        log "❌ stuck at ${remaining}: at 100 or more, unresolvable pools fill yog-context's batch and starve the rest"
        return 1
      fi
      log "⚠️  ${remaining} pool(s) keep their flag: yog-context cannot resolve them (closed on chain or rejected; its WARN lines name them)"
      return 0
    fi
    sleep 15
  done
}

main "$@"; exit
