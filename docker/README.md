# Deploying

Production runs a **version**: a tag `vX.Y.Z` on `main`. Nothing else is
deployed — not `main` as it stands, not a branch, not a bare commit. This file
is the rule; the two scripts below are the mechanism, and their headers are
where the mechanism is written down.

- `update.sh` — the update procedure, run on the server: `update.sh vX.Y.Z`,
  `update.sh --force vX.Y.Z`, `update.sh --check`. It detaches, builds while
  the previous version serves, stops the daemons, migrates, starts, checks.
- `deploy-entry.sh` — the forced command of the deployment key: the only three
  forms the pipeline may ask for.

The pipeline is the `deploy` workflow (`.github/workflows/deploy.yml`), run by
hand from GitHub's Actions tab. It only opens an SSH session to that forced
command.

## When to cut a version

A version is a batch of merged pull requests, not one per merge: each deploy
costs a build of several minutes on the production host and an outage of a
few seconds. Cut one:

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
   request was already held to its own CI before it could merge; this is the
   run on `main` itself, after the merge.
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
4. Run the `deploy` workflow, mode `deploy`, with that version.
5. Check: the run ends on `✅ deployed and checked`, and the Healthchecks.io
   checks stay green. `--check` (mode `check`) names the version deployed.

`force` runs the whole sequence for a version already deployed — to apply a
change of the server's `.env`, for instance.

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
scripts would put back the gate that only knew `main`.
