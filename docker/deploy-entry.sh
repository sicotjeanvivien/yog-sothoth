#!/usr/bin/env bash
# docker/deploy-entry.sh — the only thing the deployment key can run.
#
# The key of the deployment pipeline (.github/workflows/deploy.yml) is
# declared on the server with this script as its forced command: whatever the
# client asks for arrives here in SSH_ORIGINAL_COMMAND, and only three forms
# go through to docker/update.sh — a version `vX.Y.Z`, `--force vX.Y.Z`, and
# `--check`. Anything else is refused before anything runs. The `docker` group
# the account belongs to is root-equivalent: this list, not the account, is
# what bounds the key.
#
# Setting it up (outside the repository, once)
# --------------------------------------------
# 1. A dedicated key, without passphrase — only GitHub will hold it:
#      ssh-keygen -t ed25519 -N "" -C github-deploy-yog-scope -f ~/.ssh/yog_deploy_ed25519
# 2. On the server, one line in the deploying account's ~/.ssh/authorized_keys:
#      restrict,command="/srv/yog-sothoth/docker/deploy-entry.sh" ssh-ed25519 AAAA… github-deploy-yog-scope
#    `restrict` takes away the pty, port and agent forwarding and X11; the
#    command makes this script the only program the key starts.
# 3. The server's host key, pinned. Read it from outside and compare its
#    fingerprint with the one the server itself gives — a keyscan alone
#    trusts whoever answers:
#    <host> is the server's IP, not a domain name: a name behind the
#    Cloudflare proxy reaches Cloudflare, not port 22, and the pinned entry
#    must match DEPLOY_HOST's exact form.
#      ssh-keyscan -t ed25519 <host> > known_hosts && ssh-keygen -lf known_hosts
#      (on the server) ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
# 4. A `production` environment that only `main` may deploy from, holding
#    the three secrets — so a copy of the workflow edited on another branch
#    does not get the key:
#      gh api -X PUT repos/<owner>/<repo>/environments/production \
#        -F 'deployment_branch_policy[protected_branches]=false' \
#        -F 'deployment_branch_policy[custom_branch_policies]=true'
#      gh api -X POST repos/<owner>/<repo>/environments/production/deployment-branch-policies -f name=main
#      gh secret set DEPLOY_SSH_KEY     --env production < ~/.ssh/yog_deploy_ed25519
#      gh secret set DEPLOY_KNOWN_HOSTS --env production < known_hosts
#      gh secret set DEPLOY_HOST        --env production --body 'jv@<host>'
#    then delete the private key locally: nothing but the pipeline needs it.
#
# The list below is the gate: what the key may ask for. update.sh's own
# parser defines the modes, and the workflow's `options` is only its menu. A
# new mode of update.sh is added here too — on purpose: the gate stays an
# explicit list, never "whatever update.sh accepts".
#
# The forced command points at this file in the checkout: it moves with each
# version checked out, like the script it guards. Moving the checkout means editing
# the authorized_keys line by hand.
set -euo pipefail

# The whole string is matched, anchored at both ends, and the arguments are
# rebuilt from the match — never by splitting what the client sent. `$` in a
# bash regex anchors the end of the string, not of a line, so a newline
# cannot smuggle a second command past it. Whether the tag exists, and is on
# main, only update.sh can tell: it refuses those before changing anything.
cmd=${SSH_ORIGINAL_COMMAND:-}
version='v[0-9]+\.[0-9]+\.[0-9]+'
if [[ $cmd == "--check" ]]; then
  args=(--check)
elif [[ $cmd =~ ^($version)$ ]]; then
  args=("${BASH_REMATCH[1]}")
elif [[ $cmd =~ ^--force\ ($version)$ ]]; then
  args=(--force "${BASH_REMATCH[1]}")
else
  echo "deploy-entry: refused: '${cmd}' (accepted: vX.Y.Z, --force vX.Y.Z, --check)" >&2
  exit 2
fi

exec "$(dirname "$(readlink -f "$0")")/update.sh" "${args[@]}"
