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

The images are public packages of GHCR, `ghcr.io/sicotjeanvivien/yog-*`: the
server pulls them without credentials. They hold nothing the public repository
does not: no `.env` reaches a build (`.dockerignore`), and the one build
argument, the browser's API URL, is public by nature.

Set up once:

- the repository variable `YOG_API_DOMAIN` (Settings → Secrets and variables
  → Actions → Variables), e.g. `api.yog-scope.xyz`. The dashboard's API URL
  is written into its image at build time: changing the domain takes a new
  version.
- after the first `images` run, each of the eight packages made **public**
  (package settings → Change visibility). A package is born private, and the
  change cannot be undone. `yog-buildcache`, the build cache, stays private:
  only the workflow reads it.

### A new server

Nothing is built on the server, so a first `up -d` needs the images first:

```bash
docker/pull-images.sh ghcr.io/sicotjeanvivien vX.Y.Z "$(git rev-parse vX.Y.Z^{commit})" \
  $(docker compose config --images | grep ':prod$')
```

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
