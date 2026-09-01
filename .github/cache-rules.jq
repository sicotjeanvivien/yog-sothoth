# Ce que le ménage hebdomadaire supprime sur `refs/heads/main`, une règle par
# famille de cache. Fichier séparé pour qu'il soit exécutable tel quel en local
# — c'est ainsi qu'il a été éprouvé :
#
#   jq --argjson budget 2147483648 -f .github/cache-rules.jq caches.json
#
# où `caches.json` vient de :
#   gh api --paginate "repos/<o>/<r>/actions/caches?per_page=100" \
#     -q '.actions_caches[]' | jq -s '.'
#
# Les trois formats de clé ont été relevés le 1er septembre 2026, pas supposés :
#
#   v0-rust-<job>-<os>-<outils>-<deps>     rust-cache, DEUX hachages
#   node-cache-<os>-<arch>-npm-<lock>      setup-node, UN seul
#   buildkit-blob-1-sha256:<64 hex>        un par blob de couche
#   index-buildkit-1-<8 hex>#1             le manifeste qui les relie

# Une génération par identité de job : on retire les $strip derniers segments de
# la clé, on trie du plus récent au plus ancien, on garde le premier.
def gens($prefix; $strip):
  [ .[] | select(.key | startswith($prefix)) ]
  | group_by(.key | sub($strip; ""))
  | map(sort_by(.created_at) | reverse)
  | { keep: map(.[0]), drop: (map(.[1:]) | add // []) };

# Portée entière : rien ne permet de rattacher un blob à son index, donc on
# garde tout, ou on supprime index et blobs ensemble quand le budget est dépassé.
def scope($prefixes; $budget):
  [ .[] | select(.key as $k | $prefixes | any(. as $p | $k | startswith($p))) ]
  | (map(.size_in_bytes) | add // 0) as $used
  | if $used > $budget
    then { keep: [], drop: ., used: $used, over: true }
    else { keep: ., drop: [], used: $used, over: false } end;

[ .[] | select(.ref == "refs/heads/main") ]
| { rust:     gens("v0-rust-";    "-[^-]+-[^-]+$"),
    node:     gens("node-cache-"; "-[^-]+$"),
    buildkit: scope(["buildkit-blob-", "index-buildkit-"]; $budget) }
| . + { drop: (.rust.drop + .node.drop + .buildkit.drop),
        keep: (.rust.keep + .node.keep + .buildkit.keep) }
