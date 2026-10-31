# Deploying

Production runs a **version**: a tag `vX.Y.Z` on `main`. Nothing else is
deployed — not `main` as it stands, not a branch, not a bare commit. This file
is the rule; the files below are the mechanism, and their headers are where
the mechanism is written down.

- `docker-bake.hcl` — the eight production images of a version, as the
  `images` workflow (`.github/workflows/images.yml`) builds them when a tag is
  pushed, and pushes them to GHCR, labelled with the tag's commit.
- `update.sh` — the update procedure, run on the server: `update.sh vX.Y.Z`,
  `update.sh --force vX.Y.Z`, `update.sh --check`. It detaches, pulls the
  version's images while the previous version serves, stops the daemons,
  migrates, starts, checks. Nothing is built on the server.
- `pull-images.sh` — the pull: it refuses an image that is missing or labelled
  with another commit, before anything is stopped.
- `deploy-entry.sh` — the forced command of the deployment key: the only three
  forms the pipeline may ask for.
- `observe.sh` — what the observation key runs: it reads the state of
  production and changes nothing (*The observation key*, below).

The pipeline is the `deploy` workflow (`.github/workflows/deploy.yml`), run by
hand from GitHub's Actions tab. It only opens an SSH session to that forced
command.

## When to cut a version

A version is a batch of merged pull requests, not one per merge: each deploy
costs an outage of a few seconds. Cut one:

- as soon as `main` holds a change to what runs — code, images, compose,
  migrations — and at the latest at the end of a working session, so that
  production never lags `main` for long;
- **at once, alone**, for an urgent fix.

A change that runs nothing (a README, a test, the CI) waits for the next one.

## Numbering

`v0.MINOR.PATCH`: `MINOR` for something new, `PATCH` for a fix. The numbers
may climb as high as they like. The public changelog
(`web/src/components/marketing/changelog/releases.ts`) uses the same names.

**A tag never moves.** A version names one commit for good: a mistake is
fixed by the next version, never by re-tagging. `update.sh` checks the tag
against origin before deploying it: one that names another commit there is
refused rather than followed, and one deleted there is not deployed.

**A tag goes on main itself** — a commit of main's first-parent line, in
practice the merge commit of a pull request — never on a commit of a merged
branch, which would miss whatever was merged beside it. `update.sh` refuses
the latter.

## Cutting and deploying one

1. Check that the CI of `main` is green on the commit to release. Each pull
   request was held to the CI of the paths it touched before it could merge
   — the crates, the dashboard and its image, the shell scripts and the
   workflows. Not the backend images: the `images` run of the tag is their
   first build.
2. Do what the merged pull requests list under **Avant de déployer** (see
   below) — first, because the deploy cannot be trusted to notice. A
   variable the production overlay requires with `${VAR:?…}` stops it at the
   first compose call, before stopping anything; a bare `${VAR}` or a
   `${VAR:-}` only warns, out of the pipeline's log, and the daemon that
   needs it then fails at start, after the outage.
3. Tag and push:
   ```bash
   git tag vX.Y.Z <commit on main>
   git push origin vX.Y.Z
   ```
4. Wait for the `images` run of that tag to go green. A deploy started before
   stops at the pull, with the previous version still serving. A run that
   failed is started again by hand, with the version (`workflow_dispatch`).
5. Run the `deploy` workflow, mode `deploy`, with that version.
6. Check: the run ends on `✅ deployed and checked`, and the Healthchecks.io
   checks stay green. `--check` (mode `check`) names the version deployed.

`force` runs the whole sequence for a version already deployed — to apply a
change of the server's `.env`, for instance.

## The registry

The images are public packages of GHCR, `ghcr.io/sicotjeanvivien/yog-*`, the
build cache `yog-buildcache` included: a package pushed by the workflow of this
public repository is born public, and a public package cannot be made private
again. The server pulls them without credentials.

⚠️ Whatever enters a build is published the moment it is pushed. A build sees a
checkout of the repository, so no `.env` reaches it (`.dockerignore` keeps it
out of a local build too); no secret is passed to it; and its one build
argument, the browser's API URL, is public by nature. A secret must never be
given to a build, as an argument, a variable or a mounted file.

Set up once: the repository variable `YOG_API_DOMAIN` (Settings → Secrets and
variables → Actions → Variables), e.g. `api.yog-scope.xyz`. The dashboard's API
URL is written into its image at build time: changing the domain takes a new
version.

### A new server

Nothing is built on the server, so a first `up -d` needs the images first:

```bash
docker/pull-images.sh ghcr.io/sicotjeanvivien vX.Y.Z "$(git rev-parse vX.Y.Z^{commit})" \
  $(docker compose config --images | grep ':prod$')
```

## The observation key

A key that reads production and can do nothing else, so that its state is
read without the account that deploys: `ssh yog-obs health`, `metrics` or `db`.

- `health` — each service's state, start time, restarts and OOM kill; its
  memory against its limit; its `WARN` and `ERROR` lines of the last hour,
  in JSON or in text, **counted**; the host's disk and memory; and whether the installed script
  still matches the checkout's.
- `metrics` — the `yog_*` series of the four daemons, read from inside the
  compose network through yog-web's `wget`.
- `db` — fixed counts, as `yog_api`, read-only and bounded to 15 s a query.
  `jobs_last_run_failed` counts `yog_migrate`'s jobs whose last run failed: one
  that gave up on a lock during a dump is expected, and retries by itself. A count
  that stays above 0 day after day is the alarm no daemon raises.

Two barriers, each enough on its own to keep root out of reach:

1. **The user can do nothing.** `yog-obs` is in no group (no `docker`, no
   `sudo`), and its `authorized_keys`, owned by root, forces
   `/usr/local/sbin/yog-observe` with `restrict`: no shell, no forwarding.
   Run as `yog-obs`, the script is a gate: `health`, `metrics` or `db`,
   matched whole, or a refusal with code 2 before anything runs.
2. **`sudo` allows three exact forms.** The gate hands the accepted word to
   `sudo -n /usr/local/sbin/yog-observe <word>`, the only commands the sudoers
   rule allows; run as root, the script checks the word again, then reads.

Nothing it prints is a secret or a text someone else wrote. It never opens
the `.env` itself, and what compose reads of it never reaches the client: root's
errors are discarded, since compose quotes the line of a `.env` it cannot
parse, and a reading that fails says so on stdout. `docker inspect` only ever
runs with a fixed template; log lines
are only counted, since one can quote a token name anyone may publish;
`metrics` and `db` lines are kept only if they match a strict pattern, and the
others are counted (`refused_lines`).

The installed copy is the one that runs, not the checkout's: a deploy does not
change what the key can do. A version that changes `observe.sh` says, under
*Avant de déployer*, to install it again; `health` reports `differs` until it
is.

### Setting it up (once, as root on the server)

```bash
adduser --disabled-password --gecos '' yog-obs     # no group added: that is the point
install -o root -g root -m 755 /srv/yog-sothoth/docker/observe.sh /usr/local/sbin/yog-observe

# the sudoers rule, checked before it is put in place — in a file only root can open, never a
# fixed name in /tmp that anyone may create first
rule=$(mktemp)
cat >"$rule" <<'EOF'
yog-obs ALL=(root) NOPASSWD: /usr/local/sbin/yog-observe health, /usr/local/sbin/yog-observe metrics, /usr/local/sbin/yog-observe db
EOF
visudo -cf "$rule" && install -o root -g root -m 440 "$rule" /etc/sudoers.d/yog-obs; rm -f "$rule"

# the key, in a file yog-obs cannot rewrite — nor anything in its home: it could rename `.ssh`
# away, or put code in the `.bashrc` that bash sources for sshd before the forced command
chown -R root:root /home/yog-obs && chmod 755 /home/yog-obs
install -d -o root -g root -m 755 /home/yog-obs/.ssh
echo 'restrict,command="/usr/local/sbin/yog-observe" ssh-ed25519 AAAA… claude-observe-yog-scope' \
  >/home/yog-obs/.ssh/authorized_keys
chown root:root /home/yog-obs/.ssh/authorized_keys && chmod 644 /home/yog-obs/.ssh/authorized_keys

sshd -T | grep -iE '^allow(users|groups)'           # if either is set, add yog-obs to it
```

On the machine that reads, the key has no passphrase (it is used without
anyone at the keyboard) and is pinned to its host entry:

```bash
ssh-keygen -t ed25519 -N "" -C claude-observe-yog-scope -f ~/.ssh/yog_observe_ed25519
cat >>~/.ssh/config <<'EOF'
Host yog-obs
    HostName <the server's IP>
    User yog-obs
    IdentityFile ~/.ssh/yog_observe_ed25519
    IdentitiesOnly yes
EOF
```

What it may never do, even when asked: write, restart or deploy (the
deployment key does that), open a shell, run free SQL. A reading that comes
back often becomes a word of this script, reviewed in a pull request. To
revoke it, delete its line in `/home/yog-obs/.ssh/authorized_keys`.

## Avant de déployer

Every pull request says, under **Avant de déployer** (the section of
`.github/pull_request_template.md`), what production needs before its code
arrives: a variable or a secret in the server's `.env`, a Healthchecks.io
check, a command on the server — and its migration, if any. `rien` when there
is nothing: the section is never left out, because an empty section and a
forgotten one read the same.

A migration is listed even when nothing has to be done for it: it is what
makes the version impossible to roll back.

## Going back

Deploy the previous version. `update.sh` checks it out and runs the same
sequence, unless the database holds a migration that version does not ship —
it reads `_sqlx_migrations`, not git. It then refuses, naming the migrations,
because the schema cannot go back with the code and the older binaries refuse
a schema newer than theirs. Past an applied migration, the only way is forward
— a fix, in a new version — or a restore (`crates/persistence/README.md`,
*Backup and restore*).

The same goes after a failed deploy: if its migration was not applied, the
previous version deploys again; if it was, it is refused, and a fix goes
forward. A version from before versioned deploys is refused outright — its
scripts would put back the gate that only knew `main`. A version from before
the registry (v0.1.10 and earlier) has no images there: its own `update.sh`
builds them on the server, as it did then — and its overlay still requires
`YOG_API_DOMAIN` in the server's `.env`.
