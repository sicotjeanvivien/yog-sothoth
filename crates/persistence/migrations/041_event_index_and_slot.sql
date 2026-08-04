-- ============================================================================
-- 041 — la position d'un event dans la chaîne : slot, event_index,
--       transaction_index, et la clé d'unicité qui s'appuie dessus
-- ============================================================================
-- Une transaction qui traverse plusieurs pools (route multi-sauts) émet un
-- event par saut. La clé unique des 19 tables d'events était
-- `(signature, timestamp)` — sans discriminant intra-transaction — et l'INSERT
-- est `ON CONFLICT DO NOTHING` : tout sauf le premier arrivé était jeté, sans
-- erreur, sans log, sans métrique.
--
-- Mesuré le 4 août 2026 sur trois pools mainnet et 482 émissions : 29 pertes
-- imputables à la clé, 0 au transport. Le taux croît avec la centralité du pool
-- dans le routage — 7,96 % sur SOL-USDC, 5,81 % sur NEST-SOL, 3,39 % sur
-- AVICI-USDC. Le défaut frappe donc le plus fort là où les données comptent le
-- plus, et il est le plus discret sur les pools de queue.
--
-- ## Ce que la correction achète, et ce qu'elle n'achète pas
--
-- La **complétude**, pas le volume. Les jambes perdues pèsent 0,04× la moyenne
-- (1 $ contre 32 $) : elles font ~8 % des events pour ~0,4 % du volume. Ne pas
-- lire ces pourcentages comme un déficit de chiffre d'affaires.
--
-- ## `event_index` indexe les payloads bruts, pas les events reconnus
--
-- Il numérote les self-CPI Anchor de la transaction telles que
-- `extract_anchor_event_cpis` les rend, y compris celles dont le discriminateur
-- n'est pas (encore) implémenté. Indexer les seuls events reconnus ferait
-- décaler tous les index déjà en base le jour où un discriminateur de plus est
-- décodé, et un rejeu insérerait des doublons au lieu d'être idempotent.
-- Corollaire : le filtre de `try_extract_self_cpi_data` est figé par contrat.
--
-- ## Les lignes existantes : `0`, puis un rang là où `0` ne suffit pas
--
-- `slot` et `event_index` sont `NOT NULL DEFAULT 0` le temps du remplissage,
-- puis le DEFAULT est retiré. `0` est honnête sur les quatorze tables dont la
-- clé était `(signature, timestamp)` : leur ancien index unique interdisait
-- déjà deux lignes par transaction, donc la survivante *était* seule.
--
-- Les cinq autres — celles qui avaient `reward_index` ou `second_position`
-- dans leur clé — hébergent légitimement plusieurs lignes par
-- `(signature, timestamp)`. Un `0` uniforme les rendrait identiques et la
-- création de l'index unique **échouerait**. Elles sont donc renumérotées par
-- `row_number()` avant. Ce rang n'est pas l'index réel sur la chaîne (il n'est
-- pas récupérable sans rejeu) : il préserve la distinction, rien de plus.
--
-- Conséquence à connaître : un rejeu d'une transaction antérieure à cette
-- migration recalculerait le vrai index et n'entrerait donc pas en conflit
-- avec la ligne rétro-numérotée — il insérerait un doublon. Aucun chemin de
-- rejeu n'existe aujourd'hui (l'ingestion est en souscription, le StreamPoller
-- dort) ; le jour où l'un est câblé, il devra partir d'après 041.
--
-- NULL n'était pas une option : il casserait la comparaison de tuples de la
-- garde d'ordre (migration suivante, ticket 04).
--
-- Le DEFAULT retiré ensuite est délibéré : un chemin d'insertion qui
-- oublierait la colonne doit échouer bruyamment, pas hériter d'un `0`
-- silencieux. C'est la leçon du `DO NOTHING` que cette migration corrige.
--
-- ## Les largeurs suivent la convention du schéma, pas la taille des valeurs
--
-- `event_index` est un `u16` en domaine, donc **INTEGER** et non SMALLINT :
-- un `u16` ne tient pas dans un SMALLINT (32 767 < 65 535), et la maison
-- range ces colonnes en INTEGER pour que la conversion à l'écriture soit
-- totale (`i32::from`) plutôt que faillible — c'est la règle que documente
-- `convert_i32_to_u16`, et que suit déjà `bin_step`. Même raisonnement d'un
-- cran plus haut pour `transaction_index`, un `u32`, en **BIGINT**. Les
-- valeurs réelles tiendraient dans deux octets ; ce n'est pas le critère.
--
-- `transaction_index` est créée **nullable et vide**. `getTransaction`
-- (Helius) ne renvoie pas le champ — vérifié en direct et sur les 6 fixtures du
-- dépôt. La clé atteignable aujourd'hui est donc `(slot, event_index)`, qui
-- ordonne dans une transaction et entre slots, mais pas entre deux transactions
-- d'un même slot. La colonne existe pour que la migration gRPC/Geyser — où la
-- mise à jour de transaction porte son `index` nativement — donne son sens au
-- champ sans seconde migration ni changement de code.
--
-- ## Les noms d'index ne sont pas codés en dur, et c'est nécessaire
--
-- Postgres tronque les noms auto-générés à 63 caractères, et deux des nôtres
-- collidaient déjà :
-- `meteora_damm_v2_update_reward_signature_reward_index_timest_idx` porte
-- `update_reward_duration_events`, et son homologue `…_times_idx1` porte
-- `update_reward_funder_events`. Le suffixe `1` dépend de l'ordre de création,
-- donc de l'ordre des migrations : un nom littéral serait juste ici et faux sur
-- une base vierge. Le bloc `DO` retrouve l'index unique portant `signature` sur
-- chaque table.
--
-- ## Les cinq discriminants existants sortent de la clé, pas de la table
--
-- `fund_reward`, `initialize_reward`, `update_reward_duration` et
-- `update_reward_funder` étaient en `(signature, reward_index, timestamp)`,
-- `split_position` en `(signature, second_position, timestamp)`. `event_index`
-- est strictement plus général : ces colonnes restent comme **données** (elles
-- portent du sens métier) mais quittent la clé, ce qui ramène 19 tables à une
-- seule règle au lieu d'en maintenir trois.
--
-- `claim_reward_events` portait `reward_index` **sans** l'avoir dans sa clé :
-- c'était une instance latente du même bug, dans une table qui peut
-- légitimement se répéter dans une transaction. Le correctif uniforme la
-- couvre.
--
-- ## Pas de GRANT ici
--
-- Les 19 tables ont déjà un `GRANT SELECT, INSERT, UPDATE … TO yog_indexer` au
-- niveau **table**, qui couvre les colonnes ajoutées plus tard. La réserve de
-- `migrations/README.md` sur les grants par colonne ne concerne que
-- `yog_context` sur `pools`, hors de cette migration.

DO $$
DECLARE
    -- Les 19 tables d'events DAMM v2. Une nouvelle table d'events créée après
    -- cette migration naît directement avec les trois colonnes et la bonne clé
    -- (cf. le gabarit de `migrations/README.md`), elle n'a rien à faire ici.
    tables CONSTANT TEXT[] := ARRAY[
        'meteora_damm_v2_claim_position_fee_events',
        'meteora_damm_v2_claim_protocol_fee_events',
        'meteora_damm_v2_claim_reward_events',
        'meteora_damm_v2_close_position_events',
        'meteora_damm_v2_create_position_events',
        'meteora_damm_v2_fund_reward_events',
        'meteora_damm_v2_initialize_pool_events',
        'meteora_damm_v2_initialize_reward_events',
        'meteora_damm_v2_liquidity_events',
        'meteora_damm_v2_lock_position_events',
        'meteora_damm_v2_permanent_lock_position_events',
        'meteora_damm_v2_set_pool_status_events',
        'meteora_damm_v2_split_position_events',
        'meteora_damm_v2_swap_events',
        'meteora_damm_v2_update_pool_fees_events',
        'meteora_damm_v2_update_reward_duration_events',
        'meteora_damm_v2_update_reward_funder_events',
        'meteora_damm_v2_withdraw_dead_liquidity_reward_events',
        'meteora_damm_v2_withdraw_ineligible_reward_events'
    ];
    tbl       TEXT;
    old_index TEXT;
BEGIN
    FOREACH tbl IN ARRAY tables LOOP
        EXECUTE format(
            'ALTER TABLE %I
                 ADD COLUMN slot              BIGINT  NOT NULL DEFAULT 0,
                 ADD COLUMN event_index       INTEGER NOT NULL DEFAULT 0,
                 ADD COLUMN transaction_index BIGINT  NULL',
            tbl);

        -- L'ancien garde d'idempotence, quelle que soit la forme qu'il avait :
        -- (signature, timestamp), (signature, reward_index, timestamp) ou
        -- (signature, second_position, timestamp). On le retrouve par ses
        -- colonnes, jamais par son nom (cf. l'en-tête).
        SELECT i.relname INTO old_index
        FROM pg_index x
        JOIN pg_class i  ON i.oid = x.indexrelid
        JOIN pg_class c  ON c.oid = x.indrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname = tbl
          AND x.indisunique
          AND EXISTS (
              SELECT 1
              FROM pg_attribute a
              WHERE a.attrelid = x.indrelid
                AND a.attnum   = ANY (x.indkey)
                AND a.attname  = 'signature'
          );

        IF old_index IS NULL THEN
            RAISE EXCEPTION
                'migration 041: aucun index unique portant signature sur %', tbl;
        END IF;

        EXECUTE format('DROP INDEX %I', old_index);

        -- Renumérotation des lignes déjà en base, sans quoi l'index unique
        -- ci-dessous ne peut pas être créé sur les cinq tables qui avaient
        -- leur propre discriminant : deux slots financés par une même
        -- transaction y cohabitent légitimement en `(signature, timestamp)`,
        -- et le `DEFAULT 0` les rendrait identiques.
        --
        -- Le rang n'est PAS l'index réel de l'event sur la chaîne — celui-là
        -- n'est pas récupérable sans rejeu. Il préserve exactement ce que
        -- l'ancienne clé garantissait : la distinction. Sur les quatorze
        -- autres tables l'ancien index unique interdisait déjà tout doublon,
        -- donc chaque groupe tient en une ligne et l'UPDATE ne touche rien.
        EXECUTE format(
            'UPDATE %I e
                SET event_index = r.ordinal
               FROM (
                   SELECT id,
                          timestamp AS ts,
                          row_number() OVER (
                              PARTITION BY signature, timestamp ORDER BY id
                          ) - 1 AS ordinal
                     FROM %I
               ) r
              WHERE e.id = r.id AND e.timestamp = r.ts AND r.ordinal > 0',
            tbl, tbl);

        EXECUTE format(
            'CREATE UNIQUE INDEX ON %I (signature, event_index, timestamp)',
            tbl);

        -- Le DEFAULT n'existait que pour remplir les lignes déjà en base.
        EXECUTE format(
            'ALTER TABLE %I
                 ALTER COLUMN slot        DROP DEFAULT,
                 ALTER COLUMN event_index DROP DEFAULT',
            tbl);
    END LOOP;
END $$;

-- ============================================================================
-- La VIEW valorisée des events de liquidité expose les trois colonnes
-- ============================================================================
-- `meteora_damm_v2_liquidity_events_valued` (migration 021) est la seule VIEW
-- que le domaine reconstruit en type d'event : son `TryFrom<Row>` doit donc
-- pouvoir remplir les trois champs. Les colonnes sont **ajoutées en fin de
-- liste** parce que `CREATE OR REPLACE VIEW` ne sait qu'étendre, jamais
-- réordonner — et les `query_as!` qui la lisent associent par position.
--
-- Les VIEW inter-protocoles (`swap_events`, `liquidity_events`, …) restent
-- inchangées : leur contrat est le jeu de colonnes commun, et aucun de leurs
-- lecteurs ne reconstruit un event typé.
CREATE OR REPLACE VIEW meteora_damm_v2_liquidity_events_valued AS
SELECT
    le.pool_address,
    le.signature,
    le.timestamp,
    le.liquidity_event_kind,
    le.amount_a,
    le.amount_b,
    le.liquidity_delta,
    le.reserve_a_after,
    le.reserve_b_after,
    le.position,
    le.owner,
    (
        (le.amount_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
      + (le.amount_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
    ) AS value_usd,
    le.slot,
    le.event_index,
    le.transaction_index
FROM meteora_damm_v2_liquidity_events le
LEFT JOIN pools p ON p.pool_address = le.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint AND fetched_at <= le.timestamp
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint AND fetched_at <= le.timestamp
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true;

-- ============================================================================
-- Vérification — la migration constate son propre résultat
-- ============================================================================
-- Le correctif porte sur 19 tables via une boucle : une table qui manquerait à
-- la liste, ou un index résiduel, se verrait en production comme une perte
-- silencieuse de plus. Autant échouer ici.
DO $$
DECLARE
    n_new INT;
    n_old INT;
BEGIN
    SELECT
        count(*) FILTER (WHERE cols = ARRAY['signature', 'event_index', 'timestamp']),
        count(*) FILTER (WHERE cols <> ARRAY['signature', 'event_index', 'timestamp'])
    INTO n_new, n_old
    FROM (
        SELECT (
            SELECT array_agg(a.attname::TEXT ORDER BY k.ord)
            FROM unnest(x.indkey::SMALLINT[]) WITH ORDINALITY AS k(attnum, ord)
            JOIN pg_attribute a
              ON a.attrelid = x.indrelid AND a.attnum = k.attnum
        ) AS cols
        FROM pg_index x
        JOIN pg_class c     ON c.oid = x.indrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname LIKE 'meteora\_damm\_v2\_%\_events'
          AND x.indisunique
          AND NOT x.indisprimary
    ) AS unique_indexes;

    IF n_new <> 19 OR n_old <> 0 THEN
        RAISE EXCEPTION
            'migration 041: attendu 19 index (signature, event_index, timestamp) et 0 résiduel, obtenu % et %',
            n_new, n_old;
    END IF;
END $$;
