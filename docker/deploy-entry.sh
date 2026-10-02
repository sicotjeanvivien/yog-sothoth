#!/usr/bin/env bash
# docker/deploy-entry.sh — the only thing the deployment key can run.
#
# The key of the deployment pipeline (.github/workflows/deploy.yml) is
# declared on the server with this script as its forced command: whatever the
# client asks for arrives here in SSH_ORIGINAL_COMMAND, and only three values
# go through to docker/update.sh — nothing, `--force`, `--check`. Anything
# else is refused before anything runs. The `docker` group the account belongs
# to is root-equivalent: this list, not the account, is what bounds the key.
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
#      ssh-keyscan -t ed25519 <host> > known_hosts && ssh-keygen -lf known_hosts
#      (on the server) ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
# 4. Three repository secrets:
#      gh secret set DEPLOY_SSH_KEY     < ~/.ssh/yog_deploy_ed25519
#      gh secret set DEPLOY_KNOWN_HOSTS < known_hosts
#      gh secret set DEPLOY_HOST --body 'jv@<host>'
#    then delete the private key locally: nothing but the pipeline needs it.
#
# The forced command points at this file in the checkout: it is updated by the
# fast-forward like the script it guards. Moving the checkout means editing
# the authorized_keys line by hand.
set -euo pipefail

case "${SSH_ORIGINAL_COMMAND:-}" in
  "") args=() ;;
  --force | --check) args=("$SSH_ORIGINAL_COMMAND") ;;
  *)
    echo "deploy-entry: refused: '${SSH_ORIGINAL_COMMAND}' (accepted: nothing, --force, --check)" >&2
    exit 2
    ;;
esac

exec "$(dirname "$(readlink -f "$0")")/update.sh" "${args[@]}"
