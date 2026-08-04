-- ============================================================================
-- 042 — pool_current_state cesse d'ordonner à la seconde
-- ============================================================================
-- La garde de l'upsert de projection comparait `last_event_at`, un TIMESTAMPTZ
-- issu du `blockTime` — donc une **seconde**, en strict. Or 56,1 % des swaps
-- partagent leur `(pool, timestamp)`, jusqu'à 46 dans la même seconde :
-- l'audit du 3 août 2026 a mesuré **33,5 % des mises à jour d'état rejetées**,
-- et étiquetées « stale » comme s'il s'agissait de concurrence saine. Ce n'en
-- était pas : c'était la granularité de la garde.
--
-- Conséquence la plus visible, confirmée en revue de la PR #96 : les deux
-- jambes d'une transaction routée sont toutes deux persistées, mais c'est la
-- **première** qui gagne la projection — l'état final de la transaction est
-- jeté, et le pool affiche des réserves et un `sqrt_price` intermédiaires.
--
-- Ces trois colonnes reçoivent la position de l'event qui a produit l'état.
-- La garde les compare en tuple (voir `repositories/pool_current_state.rs`) :
-- `last_event_at` reste, comme donnée d'affichage, mais cesse d'être la clé
-- d'ordre.
--
-- ## L'ordre obtenu est partiel, et c'est mesuré plutôt que masqué
--
-- `getTransaction` ne renvoie pas `transaction_index` (cf. migration 041), donc
-- deux transactions d'un même slot touchant le même pool sont départagées par
-- le seul `event_index` — ce qui peut se tromper dans les deux sens : rejeter
-- un event réellement postérieur, ou en accepter un réellement antérieur.
--
-- La garde s'écrit avec `COALESCE(last_transaction_index, 0)` pour que la
-- migration gRPC/Geyser — où la mise à jour de transaction porte son `index`
-- nativement — rende l'ordre total **sans migration ni changement de code**.
-- En attendant, le cas est compté :
-- `yog_indexer_pool_current_state_same_slot_total`.
--
-- ## Les largeurs suivent le type de domaine, pas la magnitude
--
-- Même règle qu'en 041 : `last_event_index` est un `u16` donc INTEGER (un `u16`
-- ne tient pas dans un SMALLINT), `last_transaction_index` un `u32` donc
-- BIGINT. Les conversions à l'écriture restent totales.
--
-- ## `0` sur les lignes existantes
--
-- `DEFAULT 0` le temps du remplissage, puis retiré — un chemin d'écriture qui
-- oublierait la colonne doit échouer bruyamment plutôt qu'hériter d'un `0`
-- plausible. Et `0` n'est pas plausible : c'est le slot du genesis de Solana
-- (juin 2020) quand cp-amm est déployé en 2025 et que les slots réels tournent
-- autour de 300 millions. Une valeur impossible, donc une sentinelle : toute
-- ligne d'avant cette migration sera dépassée par le premier event qui arrive,
-- ce qui est exact — son état vient d'une garde qu'on est en train de corriger.
--
-- Pas de renumérotation ici, contrairement à 041 : `pool_current_state` a une
-- ligne par pool, rien ne peut collider.
--
-- ## Pas de GRANT
--
-- `yog_indexer` détient déjà `INSERT, SELECT, UPDATE` au niveau **table** sur
-- `pool_current_state` (cf. `tests/privileges.rs`), ce qui couvre les colonnes
-- ajoutées plus tard. La matrice de privilèges est inchangée.

ALTER TABLE pool_current_state
    ADD COLUMN last_slot              BIGINT  NOT NULL DEFAULT 0,
    ADD COLUMN last_event_index       INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN last_transaction_index BIGINT  NULL;

-- Le DEFAULT n'existait que pour remplir les lignes déjà en base.
ALTER TABLE pool_current_state
    ALTER COLUMN last_slot        DROP DEFAULT,
    ALTER COLUMN last_event_index DROP DEFAULT;
