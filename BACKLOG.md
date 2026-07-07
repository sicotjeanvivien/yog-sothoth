# Yog-Sothoth — Backlog

> Source of truth opérationnelle. Mettre à jour en fin de session / fin de journée.
> Statuts : `[ ]` à faire · `[~]` en cours · `[x]` ✅ fait · `[-]`  🚫 abandonné (raison entre parenthèses · 🔜 reporté ultérieurement )

---
---
## ✅ v0.1 — Analyzer + Signal Engine

> Décision (10 juin 2026) : v0.1 et v0.2 fusionnées. Pas de release publique tant qu'il n'y a pas de signaux à offrir — un analytics Solana sans détecteurs est un viewer d'events, pas un produit. Le découpage interne (v0.1.0 / v0.1.1) reste pour conserver l'ordre de construction.

---
### ✅ v0.1.0 — Analyzer (POC, pas de release publique)

#### ✅ Indexer — Cercle 2 events
> Fondation per-protocole en place (voir section ✅ refactor voie 3 ci-dessous). Les cinq events s'ajoutent en suivant le pattern : wire event borsh → discriminator + extractor → translator → variant `MeteoraDammV2Event` → struct domaine + repo trait → table `meteora_damm_v2_<event>_events` + VIEW → bras `MeteoraDammV2EventPersistor`.

- [x] `EvtInitializePool` — débloque `fee_tier` dans `PoolResponse`
- [x] `EvtCreatePosition`
- [x] `EvtClosePosition`
- [x] `EvtLockPosition`
- [x] `EvtPermanentLockPosition`
- [x] `EvtSetPoolStatus`
- [x] `EvtUpdatePoolFees`
- [x] Test cercle 2 avec fixture ( non fait pour le `EvtSetPoolStatus` car pas d'event sur SOLSCAN  )
- [x] Activer `fee_tier` dans `PoolResponse` une fois `EvtInitializePool` indexé → champ `feeBps` (base fee en bps), porté par la colonne `pools.fee_bps` (migration 015)
- [x] **Résolution `fee_bps` depuis le compte Pool (yog-context)** : l'event-only laissait `fee_bps` NULL pour les pools créées avant le démarrage de l'indexer. `PoolAccountWorker` (ex-`PoolMintsWorker`, généralisé) lit `cliff_fee_numerator` (u64 @ offset 8, validé empiriquement sur mainnet) en même temps que les mints et back-fill tout `fee_bps` NULL. `getFeeForMessage` = fausse piste (frais réseau, pas frais de pool). L'event-driven `set_fee_bps` de l'indexer est conservé (forward/live, seul à rafraîchir un fee déjà posé sur `UpdatePoolFees`)
- [x] `MeteoraDammV2InitializePoolEvent::pool_fees_raw` à décoder → base fee décodé (`core::amm::damm_v2::decode_base_fee_bps`, mode-aware, fail-loud), validé sur fixtures. Octets bruts toujours stockés (voie C) ; décodage *complet* (scheduler/dynamic fee) différé jusqu'aux graphes de l'onglet Fees
- [x] `MeteoraDammV2UpdatePoolFeesEvent::params_raw` à décoder → `core::amm::damm_v2::decode_updated_base_fee_bps` lit le champ de tête `cliff_fee_numerator: Option<u64>` (robuste au drift des champs suivants : la fixture précède l'ajout de `compounding_fee_bps`), validé sur fixture (128 bps). L'indexer rafraîchit `pools.fee_bps` sur changement opérateur (Some) ; `None` = base fee inchangé. Octets bruts toujours stockés (voie C)
- [x] Sync front (schéma + affichage) : `feeBps` ajouté à `pool.ts` ; le fee tier est affiché comme ligne « Fee tier » du bloc **Pool info** du PoolDetail (`formatFeeBps` bps→%, `—` si null), i18n en/fr. Pas d'onglet dédié : le PoolDetail n'a pas de système d'onglets et `feeBps` est une seule valeur scalaire
- [x] **Fee-split config depuis le compte Pool (yog-context, PR #8)** : `{protocol,partner,referral}_fee_percent` (u8 @ offsets 48/49/50, vérifiés mainnet) résolus par `PoolAccountWorker` en même temps que mints/fee_bps ; colonnes `pools.*_fee_percent` (migration 018, GRANT UPDATE `yog_context`), exposés en `PoolResponse` + fiche pool (ligne « Fee split », PR #11). Split = constante programme Meteora (20/0/20 sur les 303 pools observées) ; seul `partner_fee_percent` peut varier (pools partenaires), `feeBps` est le seul bouton par-pool
- [x] **Fees réalisés agrégés (PR #7)** : la CA swap `meteora_damm_v2_swap_events_hourly` étendue (migration `017_swap_fee_cagg.sql`, DROP+recreate superset) avec `fee_in_a/b` + `protocol_fee_in_a/b` (total trading fee réalisé par swap = claiming+protocol+compounding+referral, splitté par `fee_token_is_a`)
- [x] **Analytics fees réalisés sur l'API (PR #9)** : `PoolAnalytics` + `GET /api/pools` exposent `fees24hUsd`/`protocolFees24hUsd`/`lpFees24hUsd`/`effectiveFeeBps` (valorisés trade-time comme `volume24hUsd`, mêmes règles de nullité). Cross-check validé : part protocole *réalisée* ≈ `protocol_fee_percent` *configuré* (~19% vs 20%)
- [x] **Web fees réalisés (PR #10)** : KPI « Fees 24h » + lignes « Effective fee (24h) » / « Protocol cut (24h) » sur le PoolDetail
- [x] **Section Fees du PoolDetail avec graphes (PRs #13 + #14)** : (1) endpoint d'historique time-series `GET /api/pools/{address}/history?days=N` (#13) — buckets horaires joignant les **4 CA** (swap fees + liquidity + claim_position_fee + claim_reward), valorisés USD trade-time, `PoolHistoryBucket`/`PoolAnalyticsRepository::history` ; (2) graphes (#14) — section serveur `PoolDetailFees` + `TimeSeriesChart` Client Component sur **visx** (revenu fee en aire, taux effectif en ligne), fenêtre 30j, i18n en/fr. Cadrage v1 acté : 2 graphes (revenu + taux effectif), liquidity/claims dans l'endpoint mais pas encore tracés. NB le barème *configuré* (decode scheduler/dynamic fee complet, cf. ligne 28) reste hors scope — on trace le réalisé

#### ✅ Dashboard — page Overview
> **Cadrage acté (18 juin 2026) : deux temps — phase 1 read-time, phase 2 = futur crate `Yog-Analytic`.**
> Coût mesuré sur la DB de dev (356 pools, 733k `token_prices`, CA swap) :
> - KPIs globaux (TVL totale, volume/fees 24h) + top-N pools par volume/TVL sont **calculables au read-time** : 5–47 ms (TVL globale 18 ms, vol/fees 24h via VIEW 019 47 ms, top-10 volume 36 ms, top-10 TVL 5,5 ms). Coût borné par le nb de pools (TVL = 1 ligne `pool_current_state`/pool) et la fenêtre 24h (chunk exclusion sur la CA), **pas** par l'historique accumulé ; lookup prix `(mint, fetched_at DESC)` index-backed → ~constant.
> - **MAIS le volume observé aujourd'hui est artificiellement contraint par l'allowlist `watched_pools`** — ce n'est pas un proxy de la charge cible. La raison d'être du projet est l'observation de protocoles **à l'échelle**. La matérialisation analytique n'est donc **pas abandonnée** : elle prendra la forme d'un crate dédié **`Yog-Analytic`** (calcul + stockage de l'analytique, forme à définir) en phase 2. La phase 1 read-time est un point de départ, pas l'état final.
> - Contraintes de design pour `Yog-Analytic` (relevées ici pour mémoire) : `pool_analytics_hourly` **ne peut pas** être un continuous aggregate (la valorisation USD joint `token_prices`, interdit en CAGG) → ce serait un `MATERIALIZED VIEW` rafraîchi (pg_cron/worker) ou une table peuplée par un worker. La TVL (état courant, pas agrégat d'events) n'est **pas backfillable** → snapshot horaire forward-only.
> - Le « mur » keyset documenté (filtres/tri TVL-volume **paginés** sur `/pools`) reste vrai pour son cas — un top-N d'Overview ne pagine pas (compute → sort → LIMIT N), donc n'y touche pas.

**Phase 1 — Overview read-time (maintenant)**
> Périmètre KPI figé (18 juin 2026) : **4 cartes scalaires**, toutes read-time. Pas de hero santé ingestion sur l'Overview (déjà présent partout dans le dashboard via le panel sidebar `network-status-panel`). Top-N pools repoussé en phase 1.5 (composant table, pas scalaire).
- [x] **Endpoint `GET /api/stats` (PR #22, en review)** : 4 KPIs globaux read-time. Nom client-agnostique (pas `/overview` : « Overview » est un écran, pas une ressource ; ship des compteurs bruts, le client dérive la couverture). Domaine séparé `core::domain::global_analytics` (`GlobalAnalytics` + repo), distinct de `pool_analytics` (per-pool) ; compteurs sur `PoolRepository::counts()` (option B : composés dans `StatsService`). Champs livrés :
	1. **TVL totale** (`totalTvlUsd`) + `poolsPriced` (numérateur de couverture ; ~349/356 ≈ 98 % en dev)
	2. **Volume 24h** (`volume24hUsd`, SUM via VIEW 019)
	3. **Fees 24h** (`fees24hUsd`, SUM fees réalisés via VIEW 019) — le différenciateur vs un simple viewer
	4. **Pools** : `poolsObserved` (COUNT cumulatif) + `poolsDiscovered24h` (COUNT `first_seen_at > NOW()-24h`)
- [x] **VIEW `pool_current_tvl` (migration 020, PR #22)** : extrait la valorisation TVL par pool (copiée-collée entre `batch_compute` et `global_analytics`) en VIEW versionnée, non préfixée (tables génériques). Dé-duplique une duplication préexistante ; même pattern que la VIEW 019.
- [x] **Implémentation front** : bande de 4 `StatCard` (`overview/page.tsx` remplace le stub) consommant `GET /api/stats` via `fetchStats`. TVL + couverture `N/M priced`, Volume 24h, Fees 24h, Pools observées + `+K découvertes (24h)` (pluriel ICU). Couverture/découverte composées côté client. Erreur fetch → `PageError`. i18n en/fr. Pas de hero ingestion (déjà dans la sidebar). Tests : schéma `StatsSchema` + `formatCount` (vitest)
- [ ] **Hors périmètre phase 1** (pour mémoire) : top-N pools (→ phase 1.5), flux récent global cross-pool (pas d'endpoint), watchlist (auth v0.2), split protocol/LP fee (redondant PoolDetail), taux de fee effectif global (trompeur — n'a de sens que par pool)

**Phase 1.5 — top-N pools (après la bande KPI)**
- [x] **Top-N pools par volume 24h** — endpoint `GET /api/pools/top?metric=volume_24h&limit=10` (read-time non-paginé, classé desc, plafonné 20 ; `metric` = enum serde-validé extensible TVL ; renvoie un `Vec<PoolResponse>` enrichi ordonné). Domaine : `PoolRankMetric` + `PoolAnalyticsRepository::top_pool_addresses` + `PoolRepository::find_by_addresses` (batch) ; `PoolService::top_pools` réimpose le rang. Front : section `OverviewTopPools` sous la bande KPI (table rang · paire · Volume 24h · TVL, lignes → fiche pool ; `BlockError` autonome si l'appel échoue, n'abat pas les KPIs), i18n en/fr. TVL en colonne mais tri volume seul pour commencer

**Phase 2 — crate `Yog-Analytic` (différé, déclenché empiriquement)**
> Items `[ ]` (crate `yog-analytic` + déclencheur empirique) regroupés dans **« Reste à faire »** en fin de fichier. Contexte/contraintes : voir le blockquote *Dashboard — page Overview* ci-dessus.

#### ✅ yog-api
- [x] `health.rs` — vérifier que ce n'est qu'une liveness, pas une readiness
- [x] **`MIDDLEWARE CORS`** — `cors_layer` passe de `permissive()` à une liste d'origines explicite. Env var **requise** `API_CORS_ALLOWED_ORIGINS` (CSV, parsée fail-loud en `Vec<HeaderValue>` dans `bootstrap::config`, tests unitaires) → `Config → run → build_router → cors_layer`. API read-only → `allow_methods([GET])`, `allow_headers([content-type])`, `expose_headers([x-request-id])` pour que le client browser remonte l'id de corrélation. SSR (`API_INTERNAL_URL`)/curl sans header `Origin` non affectés. `.env`/`.env.example`/`docker-compose` (api) renseignés (`http://localhost:3000` en dev → origine publique en prod). Vérifié en live : origine autorisée → ACAO échoué ; origine refusée → pas d'ACAO (browser bloque)
- [x] ErrorResponse RFC 9457

##### ✅ tracing HTTP
- [x] filtrage `/healthz` via `EnvFilter`
- [x] filtrage `/readyz`

#### ✅ Frontend
- [x] Copy-to-clipboard sur l'adresse Solana du wallet `support-us` — `CopyButton` promu de `dashboard/pool-detail/` vers `shared/` (2ᵉ consommateur, cross-feature) ; ajouté en îlot client dans la box wallet, la carte Sponsor reste Server Component. Clé i18n `sponsor.copyAddress` (en/fr)
- [x] Revoir /lib/api/schema — problème si valeur nul . Vérife type data possible — revue complète des 11 schémas contre les DTO Rust. Conclusion : nullabilité OK partout (les `Option<…>` Rust → `.nullable()`), `BigDecimal=string` correct (rust_decimal sérialise en string par défaut). Corrections livrées : (1) symétrie A/B des réserves + resserrements `Rfc3339`/enum ; (2) **toutes** les quantités `u64` (réserves, `amount*`, fees) → string côté API pour ne pas tronquer au-delà de 2^53 — `formatTokenAmount` accepte désormais une string et downcast à l'affichage
	- [x] api-error-body — conforme RFC 9457, RAS
	- [x] liquidity-event — `amount*` + réserves `u64` → `U128String`
	- [x] network-status — `observedAt`/`lastEventAt` resserrés en `Rfc3339`
	- [x] page — RAS
	- [x] pool-center-state (pool-current-state) — réserves `u64` → `U128String`, `lastEventKind` → `z.enum`
	- [x] pool — RAS (nullabilité + USD string déjà corrects, couverts par tests)
	- [x] price — RAS
	- [x] swap-event — `amount*` + réserves + fees `u64` → `U128String` (A/B était asymétrique : A en bigint, B en number)
	- [x] token — `logoUri` : le schéma `url|null` était **correct**, c'est l'API qui émettait `""`. Fix côté backend (yog-api normalise `""`→`null` à la sérialisation + yog-context filtre les images vides du provider Helius). Schéma front laissé strict (anti-corruption)
- [x] suppression BFF
- [x] Ajout Client Browser
	- [x] lib/api/browser/network-status.ts — browser-side, exposes fetchNetworkStatusBrowser
- [x] KPI - Current Pool Price — carte KPI « prix courant » sur la page pool, dérivée des **réserves** (convention projet : prix calculé au query-time depuis les réserves, pas le `sqrt_price`). Helper pur `computePoolPrice` + `formatPrice` (testés). Affichée en notation paire (`SOL/USDC` = prix de A en B), gated sur `state` + symboles résolus (décimales fiables) + flag `poolPriceImbalance` (enfin câblé)
	- [x] Layout KPI strip — les 5 cartes sur une rangée écrasaient le donut. Séparé en deux blocs côte à côte sur `lg` (KPICards en grille 2×2 d'un côté, `PoolCompositionCard` de l'autre), hauteurs égalisées par stretch, ratio 5/9–4/9
- [x] Enrichir `PoolCompositionCard` (volet **factuel**) — légende à deux lignes par côté : **montant USD** (`composition.valueA/BUsd`) + **réserve en unités humaines** (`formatTokenAmount` sur reserves+decimals) en plus du % existant. Réserves passées en props depuis `pool-detail-kpis` (garanties non-null car `composition` dérive de `state`). Purement présentationnel, données déjà dispo, pas de backend. typecheck/lint/140 tests verts
- [~] **Imbalance — re-scopé hors « petit item » (29 juin 2026)** : le signal annoncé (« prix implicite du pool vs oracle ») ne peut **pas** se baser sur le ratio des réserves — DAMM v2 est de la **liquidité concentrée** (`pool_current_state` porte `last_sqrt_price` + `liquidity` L, cf. `model.rs:82`), donc le ratio des réserves ≠ spot price (les réserves reflètent où la liquidité est posée vs le prix actif). Un imbalance correct doit dériver le spot price de **`sqrt_price`** au format **Q64.64** (`price = (sqrt_price / 2^64)^2`, ajusté décimales A/B), le comparer au prix oracle Jupiter (`priceAUsd/priceBUsd`), puis afficher l'écart %.
	- [x] **Spot price → core+api (décidé 29 juin 2026)** : le calcul est de la **logique de domaine** (décodage d'un encodage on-chain validé sur mainnet, comme `decode_base_fee_bps`), pas du formatage ; et l'imbalance est un **futur détecteur Signal Engine** (backend) → le calculer dans le front = double maths. Donc : helper pur `yog_core::amm::damm_v2::sqrt_price_to_price_a_in_b` (f64, validé sur 3 pools mainnet réels), exposé en `PoolCurrentStateResponse.spotPriceAInB` (`Option<Decimal>`, dérivé dans `PoolService::get_latest_state` qui résout les décimales ; **pas** sur `PoolResponse` → éviterait un N+1 dans `list_pools`/`top_pools`). Vérifié live : SOL/USDC → 71.54 (oracle 71.53 ; le ratio réserves donnait 1.30, absurde). Aucune migration/SQL. Le `computePoolPrice` (ratio réserves) côté front reste à **remplacer** par la conso de ce champ
	- [x] **Front** : la carte « Current price » consomme `spotPriceAInB` (schema `pool-current-state.ts` + `pool-detail-kpis.tsx`) ; `computePoolPrice` (ratio réserves) supprimé, `formatPrice` conservé, `pool-price.ts` réduit au formatage. Le front est purement affichage. Flag `poolPriceImbalance` conservé. typecheck/lint/136 tests verts
	- Reliquat **Imbalance %** (différé au Signal Engine) → regroupé en fin de fichier.

##### PagePool
> Reliquats `[ ]` (favoris localStorage, colonne fee + filtre) → regroupés dans **« Reste à faire »** en fin de fichier.
- [x] Tableau liquidity — colonne « Value (USD) » : valeur USD de l'événement (amountA·prixA + amountB·prixB, valorisation **trade-time** = prix as-of le timestamp de l'event). **Backend** : VIEW `meteora_damm_v2_liquidity_events_valued` (migration 021, LATERAL `token_prices` as-of + jointure décimales, GRANT `yog_api`) — les 2 chemins cursor (forward/backward) lisent la VIEW (colonnes forcées `!` car sqlx infère nullable sur une VIEW) ; read-model `MeteoraDammV2LiquidityEventValued { event, value_usd: Option<Decimal> }` (séparé de l'event brut → infra-neutral, l'INSERT indexer inchangé) ; `LiquidityEventResponse.valueUsd`. **Front** : 6ᵉ colonne, `formatUsd` plein, `—` si null. Test d'intégration VIEW (as-of correct + NULL si jambe non pricée). Vérifié live : SOL/USDC remove → $41.26. NB : `liquidityDelta` (u128 brut, unités L sans décimales) écarté car illisible.

#### ✅ yog-context — métriques
- [x] Métriques Prometheus sur worker tick metadata (10s)
- [x] Métriques Prometheus sur worker tick price (30s)

#### ✅ yog-context — découplage ports/providers et tests
- [x] Réorganisation modulaire (convention module-file) : `source/` (ports), `providers/` (adapters), `workers/` (use cases), `bootstrap/` (composition root), `error/` (vocabulaire d'erreur)
- [x] Trait `MetadataSource` (`source/metadata.rs`) — abstraction côté worker
- [x] Trait `PriceSource` (`source/price.rs`) — abstraction côté worker
- [x] `HeliusDasClient` / `JupiterPriceClient` deviennent des adapters du port respectif dans `providers/`
- [x] Chunking encapsulé dans les providers : `DAS_BATCH_MAX` / `JUPITER_BATCH_MAX` privés au module provider, méthodes du trait renommées `fetch_metadata` / `fetch_prices`, skip-and-log par chunk déplacé du worker vers le provider
- [x] Workers prennent `Arc<dyn MetadataSource>` / `Arc<dyn PriceSource>` ; câblage au type concret confiné à `bootstrap/daemon.rs`
- [x] Refacto symétrique : extraction de `into_fetched_price` dans `jupiter_price.rs` (pendant de `into_fetched_metadata` côté DAS)
- [x] Tests `providers/helius_das_tests.rs` — projection + désérialisation `DasResponse`
- [x] Tests `providers/jupiter_price_tests.rs` — projection + désérialisation `JupiterPriceEntry`
- [x] Tests `workers/metadata_tests.rs` — orchestration + résilience (skip-and-log sur chaque erreur recoverable)
- [x] Tests `workers/price_tests.rs` — orchestration + résilience + accumulation cross-chunk
- [x] Rename cross-crate `price_source` → `price_provider` (domain enum, persistence row, sqlx queries, `.sqlx` cache, migration `ALTER TABLE`)
- [x] Rename cross-crate `metadata_source` → `metadata_provider` + introduction de l'enum `MetadataProvider` colocalisé avec `TokenMetadata` (variante unique `HeliusDas`)

#### ✅ Refactor per-protocole (voie 3) — fondation pour cercle 2
> Préparation structurelle avant cercle 2 : tables et types per-protocole, dispatch propre à un seul endroit par crate, plus de mélange domaine/extraction côté yog-core.

**Indexer — nettoyage avant cercle 2**
- [x] Extraction `EventPersistor` hors de `IndexerService` (orchestration de la persistance des `DomainEvent` + projection `pool_current_state`)
- [x] Split des métriques persist : `EventPersistorMetrics` séparé d'`IndexerServiceMetrics`, fichier `indexer_service_metrics.rs` renommé pour cohérence
- [x] Extraction `TransactionFetcher` dans `infra/rpc/` (avec son `FetchError` typé déplacé avec lui) ; mesure de durée corrigée par déplacement côté caller
- [x] `IndexerService` renommé en `TransactionProcessor`, méthode `index_transaction` → `process` ; fichier `transaction_processor.rs`
- [x] Factorisation per-variant des méthodes `persist` dans `EventPersistor` (`persist_swap`, `persist_liquidity`, `persist_claim_position_fee`, `persist_claim_reward`)

**yog-core — extraction comme use case applicatif**
- [x] Dossier `protocols/` renommé en `application/extraction/` (couche application, pas domaine pur)
- [x] Renommage `extraction.rs` (file) → `outcome.rs` pour libérer le nom du mod file racine
- [x] Trait `PoolIndexer` renommé en `EventExtractor` (le trait reflète enfin ce qu'il fait — extraire des events, pas indexer un pool)
- [x] Nouvelle struct `ExtractionDispatcher` : dispatch `Protocol → handler` centralisé dans yog-core, yog-indexer ne connaît plus les handlers concrets (`MeteoraDammV2`, …)
- [x] Pré-instanciation des handlers comme champs de `ExtractionDispatcher` (plus d'`Arc<dyn PoolIndexer>` alloué par transaction)

**Schéma SQL — voie 3**
- [x] Baseline `001_initial_schema.sql` réécrit avec tables per-protocole : `meteora_damm_v2_swap_events`, `meteora_damm_v2_liquidity_events`, `meteora_damm_v2_claim_position_fee_events`, `meteora_damm_v2_claim_reward_events`
- [x] Migrations historiques 002 (check constraints) + 003/004 (renames `price_source`/`metadata_source`) fusionnées dans le baseline ; forward-only resume from this baseline
- [x] Suppression de la colonne `protocol` des tables spécialisées (l'identité de la table porte le protocole par construction)
- [x] VIEWs SQL cross-protocole : `swap_events`, `liquidity_events`, `claim_position_fee_events`, `claim_reward_events` (UNION ALL avec littéral `protocol` injecté ; prêtes pour nouvelles branches DLMM/Raydium/Orca)
- [x] Nommage harmonisé : `position_fee_claims` → `meteora_damm_v2_claim_position_fee_events`, `reward_claims` → `meteora_damm_v2_claim_reward_events`

**yog-core — DomainEvent à deux niveaux**
- [x] Restructuration `DomainEvent` : outer variant par protocole, inner sub-enum par event kind (`DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(...))`)
- [x] Nouvelle hiérarchie `domain/meteora/damm_v2/` avec sub-event modules et `damm_v2.rs` (sub-enum `MeteoraDammV2Event` + accessors)
- [x] Renames des types : `SwapEvent` → `MeteoraDammV2SwapEvent`, `LiquidityEvent` → `MeteoraDammV2LiquidityEvent`, `ClaimPositionFeeEvent` → `MeteoraDammV2ClaimPositionFeeEvent`, `ClaimRewardEvent` → `MeteoraDammV2ClaimRewardEvent` (et les `XxxRepository` traits correspondants)
- [x] Renames des cursors : `SwapCursor` → `MeteoraDammV2SwapCursor`, `LiquidityCursor` → `MeteoraDammV2LiquidityCursor` (et variants de `Cursor` enum)
- [x] Suppression du champ `protocol: Protocol` sur les sous-events (l'outer variant le porte par construction)
- [x] Adaptation du translator DAMM v2 pour produire la forme à deux niveaux

**yog-persistence — repos per-protocole**
- [x] Hiérarchie `repositories/meteora/damm_v2/` (cohérente avec yog-core)
- [x] Repos renommés : `PgSwapEventRepository` → `PgMeteoraDammV2SwapEventRepository` et al.
- [x] Row types renommés et adaptés (colonne `protocol` retirée des SELECT/INSERT)
- [x] Requêtes SQL adaptées aux nouvelles tables `meteora_damm_v2_*`
- [x] `.sqlx/` cache régénéré contre le nouveau baseline

**yog-indexer — sub-persistor per protocole**
- [x] `EventPersistor` devient un thin dispatcher qui match sur `DomainEvent::MeteoraDammV2(_)` et délègue
- [x] Nouvelle struct `MeteoraDammV2EventPersistor` qui possède les 4 repos DAMM v2 et fait son propre match sur le sub-enum
- [x] Nouvelle struct `PoolMaintenance` extraite (`upsert_pool_full`, `touch_pool`, `update_pool_current_state_from_{swap,liquidity}`), partagée par `Arc` entre tous les futurs sub-persistors
- [x] Hiérarchie `application/services/meteora/damm_v2/` (cohérente cross-crates)
- [x] Paramètre `protocol: &Protocol` retiré de `EventPersistor::persist` (implicite dans l'outer variant)

**yog-api — adaptation des services**
- [x] Renames des services internes : `swap_service.rs` → `meteora_damm_v2_swap_service.rs`, idem liquidity
- [x] Imports adaptés à travers `AppState`, DTOs, cursor, handlers
- [x] URLs publiques inchangées (`/api/pools/{addr}/swaps`, `/api/pools/{addr}/liquidity-events`) — le frontend continue d'appeler les mêmes endpoints
- [x] DTOs gardent leur shape DAMM v2-specific (next_sqrt_price, fees breakdown, liquidity_delta) ; transition vers union discriminée serde-tagged à faire le jour où un second protocole arrive

**Documentation**
- [x] `crates/README.md` mis à jour : nouveaux layouts, `EventExtractor`/`ExtractionDispatcher`, `TransactionProcessor`/sub-persistors/`PoolMaintenance`, section "Adding a new protocol" repensée

#### ✅ yog-api — refacto application layer (pattern PoolService)
- [x] `SwapService` avec tests unitaires (pattern identique à `PoolService`)
- [x] `LiquidityService` avec tests unitaires
- [x] `NetworkStatusService` avec tests unitaires
- [x] `TokenService`  avec tests unitaires
- [x] `PoolService` enrichissement + tests unitaires
- [x] Créations DTO/request
	- [x] GetPoolLatestStateRequest avec tests unitaires
	- [x] GetPoolRequest avec tests unitaires
	- [x] GetTokenRequest avec tests unitaires
	- [x] ListPoolLiquidityRequest avec tests unitaires
	- [x] ListPoolSwapsRequest avec tests unitaires
	- [x] ListPoolsRequest  avec tests unitaires
- [x] AppState exposes only `Arc<XxxService>`

#### ✅ yog-persistence — convention Row + TryFrom
- [x] Pattern `Row + TryFrom<XxxRow> for XxxDomain` établi sur tout le crate (parse failures → `RepositoryError::Integrity`)
- [x] `pool/` (layout hybride fichier+sous-dossier pour SQL dynamique)
- [x] `liquidity_event`
- [x] `swap_event`
- [x] `pool_current_state`
- [x] `position_fee_claim`
- [x] `reward_claim`
- [x] `token_metadata` (+ migration runtime → macros sqlx)
- [x] `token_price` (+ migration runtime → macros sqlx, `QueryBuilder` conservé pour bulk insert)
- [x] `network_status` (+ `From` → `TryFrom` : fail-loud sur conversions u64/i64 et u32/i32 au lieu de `as` silencieux)
- [x] `event_freshness` (migration runtime → `query_scalar!`)
- [x] `pool_analytics` (`TryFrom<Row> for (Pubkey, PoolAnalytics)` — tuple cible car la row porte clé+valeur)
- [x] `watched_pool`
- [x] Refacto cross-crate : `PoolCurrentState.pool_address` / `.protocol` strong-typed (`String` → `Pubkey` / `Protocol`)
- [x] Refacto cross-crate : `signature: String → solana_signature::Signature` partout (events + cursors + extract_signature au boundary RPC)
- [x] `WatchedPoolRepository::exists` / `::remove` : `&str` → `&Pubkey`


#### ✅ Yog-Persistence — maintenabilité du SQL (grosses requêtes → VIEWs)
> Douleur : certaines requêtes SQL en string Rust sont énormes et dupliquées
> (la valorisation USD trade-time était copiée dans `history` ET `batch_compute`).
>
> **Décision (17 juin 2026) : NON à un passage à SeaQuery.** Évalué et écarté :
> SeaQuery construit le SQL au runtime → on **perd la vérification schéma à la
> compilation** (le filet des macros `sqlx::query!`), et sur les requêtes
> CTE/LATERAL/window c'est *pire* (mur d'appels de builder + `Expr::cust`). Sur
> les ~13 repos statiques (INSERT/SELECT par clé) ça n'apporterait que du
> downside. Témoin réalisé sur `history` pour juger sur pièce (PR #17).
>
> **Approche retenue : extraire les grosses requêtes analytiques en VIEWs SQL
> versionnées.** Ça réduit *et* dé-duplique (composition + réutilisation), garde
> la vérification compile-time (le SELECT au-dessus reste macro-vérifié), et
> laisse le gros SQL dans du vrai fichier `.sql` outillé. Les repos statiques
> restent sur les macros sqlx, intacts.

- [x] **VIEW `meteora_damm_v2_pool_hourly_activity` (migration 019, PR #17)** : encapsule la valorisation USD par `(pool, heure)` des 4 CA. `history` passe de ~80 lignes de SQL inline à un `SELECT … FROM <view> WHERE pool/window` trivial ; `batch_compute` réutilise la même VIEW (valorisation 24h dé-dupliquée). Équivalence vérifiée (même `sum(feesUsd)`), compile-time check conservé.
- [x] **Requêtes dynamiques → on garde le `QueryBuilder` sqlx natif (décidé 17 juin 2026)**. Le SQL dynamique (`ORDER BY`/`WHERE`/search selon input user, ex. `pool/query.rs`, et les futurs filtres /pools) ne peut ni macro ni VIEW. SeaQuery évalué et **écarté même pour la couche dynamique** : pas justifié d'ajouter toute une dépendance pour 2-3 requêtes quand un `QueryBuilder` natif fait le job, contenu et testé. Acté dans CLAUDE.md → « Choosing how to write a query ».
- repos statiques (events, token_metadata, token_prices, network_status, watched_pool, …) : **on ne touche pas** — aucune douleur, les macros sqlx font le travail avec le check compile-time.

### ✅Transverse v0.1

#### ✅ Convention code/tests — un fichier de tests séparé
> Harmoniser sur le pattern déjà majoritaire : code dans `xxx.rs`, tests dans
> `xxx_tests.rs` attaché par `#[cfg(test)] #[path = "xxx_tests.rs"] mod tests;`
> (ex. `pool_analytics/rows.rs` + `rows_tests.rs`, `network_status_service.rs`
> + `tests/network_status_service_tests.rs`, `global_analytics/rows.rs` +
> `rows_tests.rs`). Garde les gros fichiers lisibles et isole les mocks de test
> du code de prod. Reste des `#[cfg(test)] mod tests { … }` inline à migrer.

- [x] `crates/indexer/src/application/services/meteora/damm_v2/event_persistor.rs` — tests + mocks (`MockPoolRepo`, `MockPcsRepo`, …) extraits dans `event_persistor_tests.rs`
- [x] Balayage workspace : **17 fichiers** `#[cfg(test)] mod tests { … }` inline séparés au pattern `#[path = "xxx_tests.rs"]` (frère, sauf `response/pool.rs` → `tests/pool_tests.rs` car le dossier a déjà un sous-dossier `tests/`). 413 tests verts, iso-comportement. **Seul `crates/wasm/src/lib.rs` laissé inline** (scaffold différé, test placeholder `it_works`, exclu du clippy) — à reprendre si/quand le crate wasm est activé

#### ✅ Stratégie de rétention & historisation (décidé : A + compression)
> **Décision (15 juin 2026) : option A — analytics only.** Au-delà de 30j, les lignes brutes
> `swap`/`liquidity` sont droppées ; l'historique long terme vit dans le rollup (CA). On ne garde
> **pas** les signatures brutes « au cas où » dans la DB chaude. L'archivage froid (audit) reste
> une option additive pour plus tard, non bloquante.
>
> Les events n'ont pas le même profil : **fort volume** (`swap`, `liquidity` — des milliers/jour)
> vs **ponctuel** (`initialize_pool`, `update_pool_fees`, `set_pool_status`, cycle de vie position).
> Objectif : DB chaude légère sans perdre l'historique. Piège à éviter : « agréger en conservant
> les signatures » est contradictoire — une agrégation écrase le grain transaction (donc la
> signature). Il faut séparer deux historiques : **analytique** (volume dans le temps → agrégé,
> signatures inutiles) et **provenance/audit** (quelles transactions exactes → grain brut, non
> agrégeable). La voie 3 aide : chaque kind est déjà sa propre table → policy par table.

- [x] **Décision A/B** — actée : **A (analytics only) + compression**. Drop des lignes brutes `swap`/`liquidity` > 30j, historique porté par le rollup CA
- [x] **Rétention différenciée par table** (migration `009_differentiated_retention.sql`) :
	- fort volume (`swap_events`, `liquidity_events`) : compression J+7 + rétention drop > 30j (inchangé)
	- ponctuel / config (`initialize_pool`, `update_pool_fees`, `set_pool_status`) : rétention **retirée** → conservés à vie ; compression conservée (reclaim sans perte)
	- cycle de vie position (`create`/`close`/`lock`/`permanent_lock_position`) : rétention **retirée** → conservés à vie ; compression conservée
	- note : 001–008 appliquaient le défaut 7d/30d uniformément ; 009 ne fait que `remove_retention_policy` sur les 7 tables ci-dessus
- [x] **Classe des `claim_*`** — actée : **gros volume, même stratégie que `swap`/`liquidity`** (rétention 30j déjà en place, conservée) → besoin du même rollup long terme avant d'activer le drop en prod
- [x] **Rollup long terme** = les 4 continuous aggregates ci-dessous (grain **horaire**, pas journalier) : portent l'historique analytique survivant au drop 30j pour `swap`/`liquidity` + `claim_*`. Migrations `010`–`013`
- [x] **Ordre d'exécution** : satisfait — les 4 CA existent (migrations `010`–`013`) ; la rétention 30j peut tourner sans perte d'historique. ⚠️ **En prod** : vérifier que la refresh policy a bien matérialisé avant qu'un chunk franchisse 30j
- [x] **GRANT** : policies (009) + CA (010–013) appliquées via `yog-migrate` ; pas de nouveau rôle requis
- [-] 🚫 🔜 **Archivage froid (plus tard, si besoin d'audit)** : dump des chunks `swap`/`liquidity` > 30j vers le bucket Object Storage `yog-backups` (parquet/csv compressé) **avant** le drop. Additif à la décision A — n'active que si un besoin de provenance/audit sur le grain transaction apparaît

#### ✅ Continuous aggregates — rollups durables (cadré, 15 juin 2026)
> Double rôle, acté avec la stratégie de rétention : (1) **historique long terme** qui survit au
> drop 30j pour les 4 tables qui droppent (`swap`, `liquidity`, `claim_position_fee`, `claim_reward`),
> (2) **perf** du calcul `volume_24h_usd` de `GET /api/pools` (aujourd'hui `SUM(...) FROM swap_events
> WHERE timestamp > NOW() - 24h`, un scan à chaque appel).

**Design acté (TimescaleDB 2.27.1) :**
- **Un CA par table source** qui droppe — même pattern × 4. Grain **horaire**, conservé à vie, `materialized_only = false` (real-time agg pour couvrir l'heure courante). Tiering horaire→journalier différé si la taille pose souci.
- **Montants bruts** dans la CA (une CAGG ne peut pas joindre `token_prices`), **conversion USD au read-time** au **prix as-of le bucket** = valorisation *trade-time* (préserve la sémantique actuelle : prix au moment du trade, pas le prix courant — l'historique ne dérive pas).
- Agrégats par `(pool_address, bucket)` (valorisation côté **entrée** du swap → sommes filtrées par direction) :
	- `swap` : `SUM(amount_a) FILTER (a_to_b)` → `volume_in_a`, `SUM(amount_b) FILTER (b_to_a)` → `volume_in_b`, `COUNT(*)`
	- `liquidity` : `liquidity_delta` est une magnitude non signée + direction `kind ∈ (add, remove)` → tout splitté par kind comme le swap par direction (`amount_a/b_added/removed`, `liquidity_added/removed`, `add_count`/`remove_count`)
	- `claim_position_fee` : `SUM(fee_a_claimed)`, `SUM(fee_b_claimed)`, `COUNT(*)`
	- `claim_reward` : `SUM(total_reward)`, `COUNT(*)`, groupé aussi par `mint_reward`
- **OHLC prix différé** : pas de `first/last/min/max(next_sqrt_price)` pour l'instant (viendra avec les courbes de prix / page Overview).
- **Sémantique 24h** : quantification horaire acceptée (« ~24 dernières heures-horloge » vs `NOW()-24h` exact) — OK pour un KPI dashboard.
- **Read mono-protocole** : `pool_analytics.rs` lira la CA swap directement ; VIEW cross-protocole *au-dessus des CA* différée jusqu'au 2ᵉ protocole.

**Contraintes migration (forward-only, sqlx lance chaque migration en transaction) :**
- `CREATE MATERIALIZED VIEW … WITH (timescaledb.continuous, timescaledb.materialized_only = false) … WITH NO DATA` (le `WITH NO DATA` évite l'erreur « CA non créable en transaction »).
- Backfill par la refresh policy, **jamais** `refresh_continuous_aggregate` dans la migration (ne passe pas en transaction).
- `add_continuous_aggregate_policy` (`start_offset` large pour backfill initial, `end_offset => '1 hour'`, `schedule_interval => '1 hour'`).
- `GRANT SELECT` sur chaque CA à `yog_api`.

**Ordre d'implémentation — `swap` en premier (slice verticale), puis réplication :**
- [x] **CA `swap`** : migration `010_swap_volume_hourly_cagg.sql` (CA + refresh policy 31j/1h + GRANT `yog_api`), réécriture sous-requête volume de `pool_analytics.rs` (lecture CA, valorisation trade-time par bucket), `.sqlx` régénéré, test d'intégration `tests/volume_cagg.rs` ✅
	- [x] Bench : plan validé via `EXPLAIN ANALYZE` (CA = lecture des buckets matérialisés + scan live du seul bucket courant via real-time agg). Latence chiffrée **déférée à la prod** : en dev (16 swaps/24h) la CA est même légèrement plus lourde (machinerie real-time agg) ; le gain n'apparaît qu'au-dessus du point de bascule (milliers de lignes/24h par pool). Cohérent avec « ne pas anticiper »
- [x] **CA `swap` étendue — fees réalisés (PR #7+#9)** : migration `017_swap_fee_cagg.sql` recrée la CA en **superset** (volume/count conservés → read-path `pool_analytics` intact, pas de regen `.sqlx` côté volume + `fee_in_a/b`, `protocol_fee_in_a/b`) ; `pool_analytics.rs` somme ces colonnes valorisées trade-time → `fees24hUsd`/`protocolFees24hUsd` sur `/api/pools`. Un CA ne peut pas `ALTER` ses colonnes → DROP+recreate (perte de l'historique matérialisé, acceptée en dev, re-backfill par la policy)
- [x] **CA `liquidity`** (historique seul) : migration `011_liquidity_hourly_cagg.sql` (split par kind, refresh policy 31j/1h, GRANT `yog_api`), test d'intégration `tests/liquidity_cagg.rs` ✅
- [x] **CA `claim_position_fee`** (historique seul) : migration `012` (`SUM(fee_a/b_claimed)`, `COUNT(*)` ; pas de mint dans la table source → jointure `pools` au read si besoin), GRANT `yog_api`, test `tests/claim_caggs.rs` ✅
- [x] **CA `claim_reward`** (historique seul, group by `mint_reward`) : migration `013` (`SUM(total_reward)`, `COUNT(*)` par `(pool, mint_reward, bucket)`), GRANT `yog_api`, test `tests/claim_caggs.rs` ✅
- [x] **Brancher des read-paths sur les CA `liquidity`/`claim_*` (PR #13)** : la requête `history` de `pool_analytics.rs` joint et lit désormais les 4 CA (swap/liquidity/claim_position_fee/claim_reward) par bucket. Côté web, seuls swap-fees sont tracés en v1 ; liquidity/claims sont exposés par l'endpoint, à tracer quand le cadrage le justifie

> Reliquat `[ ]` (**VIEW cross-protocole au-dessus des CA**, au 2ᵉ protocole) → regroupé en fin de fichier.

#### ✅ Performance — différé empirique
> N'activer que si la charge le justifie. Ne pas anticiper. Items `[ ]` (table `pool_analytics_hourly` matérialisée, cache HTTP `/api/pools`) → regroupés dans **« Reste à faire »** en fin de fichier.

#### 🚫 Infrastructure RPC — différé
- [-] 🚫 Migration vers `transactionSubscribe` Helius ou Yellowstone gRPC (Shyft/Triton) — désactive l'allowlist `watched_pools`, architecture protocol-centric pleine. À faire quand throughput devient la contrainte réelle. (Non acceptable : si mise en place => dépendance structurelle à Helius)
  - **Décision renversée le 3 juillet 2026** : l'extension multi-protocoles (v0.2) rend l'acquisition d'un flux RPC adapté **nécessaire à la viabilité du projet** → section **Pré-v0.2** en fin de fichier. L'objection « dépendance structurelle Helius » devient un *critère de choix* (multi-provider, couche subscription abstraite), pas un veto.

#### 🚫 Filtres TVL/volume sans matérialisation
- [-] 🚫 Filtres TVL min / volume min sur /pools (abandonné — TVL et volume sont calculés au read-time, pas matérialisés ; filtrage SQL efficace impossible sans table `pool_analytics_hourly` matérialisée. Repris en v0.1.1 si la table est créée)

---
---

### v0.1.1 — Signal Engine + prep release (bloque la mise en prod)

#### ✅ Signal Engine — décisions d'architecture (figées 1 juil. 2026)

Phase conceptuelle bouclée avant tout code. Décisions structurantes :

- **Concept** : un signal = une *conclusion* (forme uniforme, tous protocoles confondus), **pas** un event brut (hétérogène). C'est cette distinction qui dicte tout ce qui suit.
- **Table** : **une** table `signals` générique (hypertable sur `triggered_at`), discriminée par **deux colonnes** `detector` + `protocol` — rejoint la famille `pools`/`token_prices` (cross-protocole = table unique, protocole = colonne). **Rejeté** : tables par protocole (voie 3 — un signal est uniforme) *et* tables par détecteur `signals_<detector>` (voie 3 sur le mauvais axe : détecteurs = forte cardinalité + rotation + lecture cross-cut → le feed deviendrait un UNION ALL permanent).
- **Schéma niveau 1, sans JSONB** : colonnes communes typées (`detector, protocol, pool_address, severity, value, threshold, message, triggered_at`). Échelle d'escalade si un détecteur exige un payload structuré : (1) aucun/message → (2) `details JSONB` → (3) tables d'extension jointes par `signal_id`. On ne monte que sous preuve d'un détecteur réel. Le JSONB dans `signals` serait une exception défendable à la règle no-JSONB (qui vise les *events*), mais coûte le check sqlx compile-time + les contraintes DB sur la queue payload → si pris : promouvoir en colonne tout ce qu'on filtre/trie, garder le JSONB opaque en SQL (interprété en Rust via enum serde clée sur `detector`).
- **Modèle d'évaluation** : trait **batch à cadence par-détecteur**. Chaque `SignalDetector` déclare son `interval()` et un `evaluate(ctx) -> Vec<Signal>` qui recalcule depuis un snapshot DB — **stateless entre tics** (la DB porte l'état). Engine = une boucle de poll par détecteur, skip-and-log par détecteur, shutdown via `CancellationToken`. **Rejeté** comme substrat : le streaming (event-callback, stateful, exigerait un transport — LISTEN/NOTIFY ou couplage indexer — car `yog-signals` est un process séparé) ; les 1ers détecteurs sont à fenêtre sur des caggs déjà bucketées → le sous-seconde n'achète rien. Un trait `StreamDetector` reste ajoutable plus tard en extension, non bâti en spéculatif.
- **Injection (modèle B)** : le **détecteur porte ses repos**, injectés concrets par le binaire à la construction — exactement comme `daemon.rs` injecte `Arc::new(PgX::new(pool()))` dans chaque persistor. Les détecteurs dépendent des **traits** repo de `core`, jamais des `Pg*`. L'`EvalContext` est **fin** : il ne porte que l'horloge du tic (`evaluated_at` → `triggered_at` cohérent + fenêtres calculées depuis un point fixe). Le détecteur **rend** `Vec<Signal>` ; c'est l'**engine** qui tient le `SignalRepository` et persiste (miroir de extraction→persistor). Rejeté : le fat-context (contexte bundle tous les repos) → viole ISP/SRP.

#### ✅ Signal Engine — déroulé d'implémentation

1. [x] **Migration** : table `signals` (hypertable `triggered_at`) + rôle `yog_signals` + `GRANT` (RW `signals` → `yog_signals`, RO → `yog_api`) — migration 022, `setup_roles.sql`
2. [x] **`core`** : `Signal`/`Severity`, `SignalDetector`, `EvalContext`, `SignalRepository`, `DetectorError` — module `signals`
3. [x] **`persistence`** : `PgSignalRepository` (write) + lecture directionnelle → **décision : VIEW dédiée** `meteora_damm_v2_pool_hourly_flow` (migration 023, reprend la valorisation 019 en gardant les 2 sens séparés) + `swap_flow` core (`PoolSwapFlow`, `SwapFlowRepository`) + `PgSwapFlowRepository`. Test d'intégration `tests/swap_flow.rs`
4. [x] **`yog-signals`** : `FlowImbalanceDetector` (imbalance = (a_to_b − b_to_a)/(a_to_b + b_to_a), plancher + seuil, Warning/Critical>0.9) + `SignalEngine` (poll par détecteur, skip-and-log, `CancellationToken`). 6 tests unitaires
5. [x] **binaire `yog-signals`** (ex-`signal-engine`) : `main.rs` + `bootstrap` (Config + Daemon câblant les Pg repos), metrics Prometheus (:9000), Dockerfile + service compose (host :9002), `.env(.example)`. Vérifié live sous le rôle `yog_signals` (émet Critical ±1 / Warning). **→ fondation fonctionnelle, PR ouverte**
6. [x] **2ᵉ détecteur** : `PriceOracleDeviationDetector` — compare le spot price on-chain (décodé de `sqrt_price` Q64.64 via `sqrt_price_to_price_a_in_b`) au prix oracle (`price_a_usd/price_b_usd`), émet sur l'écart relatif `(spot − oracle)/oracle` (Warning/Critical configurables, défauts 0.05/0.2 — idem flow 0.6/0.9 ; échelle validée fail-loud au load : threshold < critical, sinon Warning inatteignable). Lecture via VIEW générique `pool_price_snapshot` (migration 024, famille 020 : tables neutres, INNER joins = seules les pools comparables) + read model core `PoolPriceSnapshot` + `PgPoolPriceSnapshotRepository`. **Gardes de fraîcheur** des deux côtés (prix oracle ≤ 15 min, dernier swap ≤ 24 h, `SIGNALS_PRICE_DEVIATION_*`) : un côté périmé = comparaison sans signification, pas une alerte — validé live (gardes off sur données figées : les pools mortes sortent à ×777). Point d'extension multi-détecteur validé : 2ᵉ boucle dans le JoinSet, zéro changement dans l'engine. Ex-reliquat v0.1 « Frontend / Imbalance % »

- [x] **Déduplication (constatée au run de l'étape 5, livrée).** Un détecteur batch ré-émettait le même signal à chaque tick. Dédup **côté engine** : cooldown glissant par `(detector, pool)` + escalade-aware. `SignalDetector::cooldown()`, `SignalRepository::latest_severity_by_pool` (`DISTINCT ON` récent par pool ; **pas** un index unique DB — TimescaleDB impose la clé de partition `triggered_at` dans les index uniques, qui diffère à chaque tick). L'engine écarte un candidat dont le pool a été signalé dans la fenêtre, **sauf** si la sévérité augmente. Stateless (la DB porte l'état). `SIGNALS_FLOW_COOLDOWN_HOURS` (défaut 6h). Vérifié live : 8 lignes/run au lieu de 24.
  - Reliquat possible (non fait) : **modèle edge/resolve** (alerte à l'entrée + « résolu » à la sortie) — plus propre en UX mais demande de tracker la résolution ; le cooldown suffit pour l'instant.

#### Signal Engine — détecteurs suivants (post-fondation)
> **Arbitrage 7 juil. 2026** : TVL drain d'abord — signal de *risque* (aligné avec flow_imbalance / price_oracle_deviation), sémantique à une seule fenêtre, données déjà en place (CA liquidity 011 + VIEW `pool_current_tvl` 020). Fee yield spike écarté pour l'instant : signal d'*opportunité* (renversement d'intention produit) et baseline à deux fenêtres → plus de tuning ; reviendra après.
- [x] **Détecteur TVL drain** (cadré et **livré 7 juil. 2026**) — détecte une pool qui se vide de sa liquidité (exode LP, comportement rug-like). Vérifié live sur la DB de dev (fenêtre élargie + seuil abaissé pour exercer le pipeline) : Warning émis à drain 0.2122 sur la pool attendue, pools sous seuil / en net inflow silencieuses, cooldown vérifié au 2ᵉ tick, card web en/fr rendue (tag TVL, % non signé — un « + » sur un drain se lirait comme une croissance). ⚠️ Relevé au passage : `pool_current_tvl` (020) n'était **pas** couverte par les default privileges de `yog_signals` (provisionnés après la 020 sur les déploiements existants) → GRANT explicite dans la 025, conforme à la règle maison (« SELECT sur une source existante = GRANT dans la migration qui introduit le besoin »).
  - **Sémantique** : sur fenêtre glissante, `drain = net_removed_usd / (tvl_actuel + net_removed_usd)` = part du TVL de début de fenêtre qui est sortie. Le **net** (remove − add) absorbe le churn des LPs qui se rebalancent. Valorisation USD as-of bucket (pattern VIEW 023).
  - **Config actée** : fenêtre **6 h** (un drain est rapide, 24 h le noierait), Warning **0.5**, Critical **0.8**, plancher TVL de départ **10 k$** (cohérent avec le plancher volume du flow), tick 300 s, cooldown 6 h. Tout en env `SIGNALS_TVL_DRAIN_*`, échelle validée fail-loud au load (threshold < critical), rien en dur.
  - **Gardes** : TVL non valorisable (prix inconnu → `tvl_usd` NULL) = pas de signal plutôt qu'un faux ; plancher sur le TVL de *départ* (tvl + net_removed), pas le TVL restant — une pool vidée à 95 % ne doit pas passer sous le plancher à cause du drain lui-même.
  - **Découpage** : migration 025 (VIEW `meteora_damm_v2_pool_hourly_liquidity_flow` sur la CA 011, GRANT yog_signals) → read model core + lens (pattern `swap_flow`) → Pg repo + test d'intégration → `TvlDrainDetector` (clone structurel de flow_imbalance) + tests unitaires → 3ᵉ boucle daemon → web (KNOWN_DETECTORS, phrasé card, panneau ⓘ, i18n en/fr).
- [ ] Détecteur Fee yield spike — signal d'opportunité (fees/TVL vs baseline) ; données prêtes (VIEWs 019+020), sémantique à deux fenêtres à cadrer le moment venu
- [ ] Détecteur Price impact creep (selon retour utilisateur — aucun à ce jour)

#### Signal Engine — canaux de diffusion (recadré 2 juil. 2026)
> **Décision** : la porte de sortie des signaux est **yog-api**, pas un pusher dans l'engine. Le canal prioritaire est le **web live** (page ouverte qui s'update à l'arrivée d'un signal) — SSE servi par l'API, alimenté par un **poller interne** (~3 s, keyset `triggered_at`, broadcast tokio vers les clients connectés). LISTEN/NOTIFY rejeté pour l'instant (contrat inter-processus, aucun gain au tempo réel des détecteurs : tick 5 min) — escalade documentée si le tempo change. **Webhook abandonné pour v0.1.1** : aucun destinataire n'existe (pas d'users avant v0.2) ; reviendra en v0.2 comme consommateur outbox per-user si besoin.

##### ✅ Canal web (focus) — 3 PRs :
1. [x] **`GET /api/signals`** — collection paginée cursor (recette add-endpoint), tri `triggered_at DESC` ; sert le rendu initial de la page et vaut seul. Livré (2 juil.) :
	1. [x] **core** : `SignalRepository::list(severity, cursor, direction, position, limit) → Page<SignalRecord>` ; curseur `SignalCursor { triggered_at, id }` (tie-breaker `id BIGSERIAL`) ; contrat d'ordre documenté (`triggered_at DESC, id DESC`) ; **modèle de lecture** `SignalRecord { id, signal }` (l'id n'existe qu'après insert, il ne va pas sur le `Signal` d'écriture) ; variante `Cursor::Signal`
	2. [x] **persistence** : `PgSignalRepository::list` — même machinerie bidirectionnelle que SwapEvent (`resolve_query_mode` + peek N+1 + `PageBuilder`, 2 requêtes statiques Forward/Backward) ; filtre severity en SQL statique (`$1 IS NULL OR severity = $1` — une égalité optionnelle n'est pas une « forme dynamique », le compile-check `query_as!` est conservé) ; `signal/rows.rs` + `rows_tests.rs` ; intégration `tests/signal_list.rs` (ordre, navigation avant/arrière, tie-break id sur timestamps égaux, positions, filtre) ; `.sqlx` régénéré
	3. [x] **api** : `SignalService` + `SignalsQuery`/`SeverityParam` (serde → 400 sur valeur inconnue) + `ListSignalsRequest` (validation avant DB) + `SignalResponse` (pubkey base58, RFC3339, `value`/`threshold` en string décimale, `id` en number) + handler + route + wiring `AppState` ; `validate_cursor_position_exclusive` extrait (partagé avec `PageQuery`)
	4. [x] **vérif live** (rôle `yog_api`, 328 signaux regénérés puis nettoyés) : page 1 → curseur → page 2, `severity=warning`, `position=last`, et les 400 RFC 9457 (limit=0, severity inconnue, cursor+position)
2. [x] **`GET /api/signals/stream`** — SSE alimenté par le poller interne. Livré (2 juil.) :
	1. [x] **core** : 2 méthodes sur `SignalFeedRepository` — `newer_than(after, limit)` (ASC strict après le curseur) + `latest_cursor()` (tip du feed, init du watermark)
	2. [x] **persistence** : impls statiques (réutilisent `SignalRow`) + extension `tests/signal_list.rs` (tip, delta ASC, cap, strictement-après) + `.sqlx` régénéré
	3. [x] **api** : `SignalStreamPoller` (couche application) — un seul poller partagé, tick `API_SIGNAL_STREAM_POLL_SECS` (défaut 3 s) ; watermark ré-ancré au tip à chaque (ré)activation → **jamais de replay** ; `receiver_count() == 0` → requête DB sautée + watermark droppé (pas de burst au retour d'un client) ; `poll_once()` testable DB-free (6 tests : ancrage, feed vide → origine, diffusion + avance, échecs skip-and-log) ; `run()` en `tokio::spawn` depuis `main` (pas de graceful shutdown côté api — meurt avec le process) ; handler SSE — `data` = `SignalResponse`, `id` = id du signal, keep-alive 15 s, `Lagged`/`Closed` → stream fermé (l'EventSource se reconnecte) ; `broadcast::Sender` dans `AppState`, `AppState::build` retourne `(state, poller)` ; `futures_util::stream::unfold` (dep workspace déjà présente, zéro ajout) ; `.env.example` + compose
	4. [x] **vérif live** : 328 signaux générés par le daemon yog-signals pendant le stream ouvert → tous reçus en direct (`data:` + `id:` + ping keep-alive) ; reconnexion → 0 replay ; CORS OK sur `text/event-stream` (ACAO echo de l'origine autorisée) ; table nettoyée
3. [x] **Page feed dans le dashboard** — la partie visible de la boucle. **Plus de BFF** → `EventSource` direct sur `/api/signals/stream`, zéro proxy. ~~NB : `CLAUDE.md` + `web/README.md` décrivent encore l'archi BFF → dérive de doc à corriger (audit ou au passage)~~ → ✅ soldé par la **refonte doc complète (PR #42, mergée 3 juil. 2026)** : root README (5 processus, roadmap réelle), `web/README.md` réécrit (direct-API, feed SSE), `crates/README.md` découpé en README par crate (`core`/`indexer`/`api`/`context`/`signals` + `persistence` étendu — le workspace README ne garde que l'inter-crates et les recettes), `CLAUDE.md` recalé (rôle `yog_signals`, clippy, carte de la doc). Règle actée : un fait vit à un seul endroit ; quand on touche un crate, son README fait partie du changement. Livré (2 juil.) :
	1. [x] **Schéma + clients** : `SignalSchema` zod (severity `z.enum`, `triggeredAt` Rfc3339, **`SignedBigDecimal` ajouté à `shared.ts`** — `value` peut être négatif, le `BigDecimal` existant est non signé à dessein) + `SignalsPageSchema` ; tests vitest (payload réel, valeur négative, sévérité inconnue rejetée, number rejeté) ; `server/signals.ts` (page 1 SSR) + `browser/signals.ts` (refill reconnexion)
	2. [x] **Hook `useSignalStream`** : EventSource direct, parse zod par event (malformé → warn + skip), `mergeSignals` **pur et testé** (dédup par id, tri `(triggeredAt, id) DESC`, cap 200 — `lib/signals/merge-signals.ts`, 4 tests) ; reconnexion auto + refill page 1 réconcilié par id sur ré-`onopen` après coupure ; état `connecting`/`live`/`reconnecting`
	3. [x] **Page + composants** : `(dashboard)/signals/page.tsx` (SSR page 1, `PageError`) ; `SignalFeed` client — badge sévérité (sky/amber/rose), tag détecteur mono, lien fiche pool (adresse courte), message (fallback valeur/seuil), horodatage relatif, pastille d'état live, état vide ; sidebar : clé + nav + **`SignalsIcon` qui existait déjà** dans la lib d'icônes
	4. [x] **i18n en/fr + vérif** : typecheck + lint + 147 tests vitest + `next build` OK ; SSR vérifié live avec données réelles (50 lignes rendues, liens pools, déviation négative affichée) ; le chemin stream lui-même vérifié end-to-end en PR #39 — la mise à jour live dans un navigateur réel reste à constater visuellement au merge ; nettoyage fait
	- Hors périmètre v1 (reliquats) : pagination UI (« charger plus » — le curseur API existe) ; filtre sévérité UI (l'endpoint le supporte)
- [x] **Refonte UX de la page Signals (constat au merge PR #40, 2 juil. 2026)** : le pipeline fonctionne de bout en bout mais l'UX front du feed d'alertes est jugée catastrophique — à reprendre en priorité à la prochaine session front (hiérarchie visuelle, lisibilité du feed, présentation des sévérités/valeurs). Occasion naturelle d'embarquer les reliquats ci-dessus (pagination UI, filtre sévérité)

##### Canaux différés :
- [ ] Telegram opérateur — après le canal web (« ensuite on verra ») ; seul push sortant avec un destinataire réel aujourd'hui (JV)
- [ ] Email — statut incertain : la forme serait un abonnement newsletter → contraintes RGPD (consentement, désinscription) ; ne se décide pas avant v0.2/auth
- [-] 🚫 Webhook v0.1.1 (abandonné — pas de destinataire ; à réévaluer en v0.2 avec users + outbox)

#### yog-context — robustesse pour release
- [ ] Worker respawn logic (actuellement abandon permanent après épuisement retry budget)
- [x] **Bug 429 Jupiter Price** (observé 6 juil. 2026, corrigé 7 juil. 2026) : les chunks de 50 mints partaient en rafale sans espacement → rate limit Jupiter → chunks suivants perdus (trous de prix). Fix : variant `SourceError::RateLimited` (détection 429 + header `Retry-After`), retry borné par chunk (3 tentatives, backoff `Retry-After` sinon exponentiel 1s/2s, cap 10s), outcome Prometheus `rate_limited`

#### yog-indexer — source de données
- [ ] Étudier le passage du WebSocket RPC à un **Yellowstone gRPC (Geyser) managé** : stream plus fiable/complet que `logsSubscribe` (reconnexions, trous). Des offres avec free tier existeraient (à vérifier : Shyft, Helius/LaserStream, QuickNode…) — comparer quotas/coûts/latence. Périmètre : seule la couche subscription de l'indexer change, le pipeline extraction → persistance reste. **→ promu en gate Pré-v0.2 (3 juil. 2026), voir section dédiée en fin de fichier** ; l'étude peut démarrer pendant v0.1.1, la migration est le gate

#### Audit complet du code — avant déploiement public
- [ ] Audit sécurité (surface API, secrets/env, rôles DB, CORS/headers, dépendances `cargo audit`/`npm audit`)
- [ ] Audit bonnes pratiques / conventions (cohérence inter-crates, erreurs typées aux frontières, couverture de tests, dette accumulée — leçon des revues PR #37 : vérifier que l'exemple copié est la règle, pas l'exception)
- [x] **Split ISP des traits repository mixtes** (relevé revue PR #38, mécanique) — livré 3 juil. 2026. L'audit exhaustif des 25 traits de `core` a élargi le scope du BACKLOG : **7 splits** (les 4 listés + `TokenMetadataRepository`, `NetworkStatusRepository`, `PoolCurrentStateRepository`, oubliés mais mêmes symptômes) + **2 méthodes mortes supprimées** (`find_by_pool` des claims — seule trace : des mocks `unimplemented!`). Convention de nommage actée (documentée dans `crates/core/README.md` → *Repository traits*) : le côté write/owner garde `*Repository`, la lunette de lecture est nommée par intention avec un vocabulaire fermé — `*Feed` (listing paginé temporel : `SignalFeed` ex-`SignalFeedRepository` renommé, `MeteoraDammV2Swap/LiquidityEventFeed`), `*Lookup` (lecture ponctuelle : `TokenMetadata/TokenPrice/NetworkStatus/PoolCurrentStateLookup`), `PoolCatalog` (surface de consultation du registre). Gain constaté : plus un seul `unreachable!()`/`unimplemented!` de stub dans les mocks (api, indexer, context). Cache `.sqlx` : 2 fichiers orphelins retirés (aucun SQL modifié)

#### Frontend — page /pools (filtres)
- [ ] Filtres TVL min / volume min — dépend de la table `pool_analytics_hourly` matérialisée (voir Reliquats v0.1 ci-dessous)

#### ✅ Frontend — mise à l'échelle globale des textes (relevé 6 juil. 2026)
- [x] Passe globale sur l'échelle typographique du dashboard (pools, pool-detail, overview, sidebar) — **livré 6 juil. 2026** (+1 cran sur tout ≤13.5px, 41 occurrences / 18 fichiers ; tailles fractionnaires 10.5/13.5 supprimées ; plancher 10px pour les micro-captions décoratives ex-9px ; valeurs KPI 21/24px display inchangées). Problème **global au front**, pas propre aux signaux : le 10–13px gris clair sur fond sombre rend flou/mou sur écran à scaling fractionnaire (constaté à 125 % Windows, cas très répandu). La card signals a servi de pilote (PR #45) : un cran partout. Barème appliqué :

  | Élément | Avant | Après |
  |---|---|---|
  | Valeur/chiffre vedette | 20px | 24px |
  | Texte principal (résumé, cellules) | 13px | 14px |
  | Texte secondaire (labels, méta) | 12px | 13px |
  | Tags mono (détecteur, etc.) | 11px | 12px |
  | Micro-tags uppercase | 10px | 11px |
  | Espacement interne | py-3, gap-1.5 | py-4, gap-2 |

  Règle retenue : **13px minimum pour le texte secondaire** ; `PoolPairCell` (14px) partagé pools/signals conforme. L'air (padding/gap) participe à la lisibilité autant que la taille.

#### ✅ Frontend — dashboard UX (relevé 6 juil. 2026)
- [x] **Overview : bloc « 5 derniers signaux »** — **livré 6 juil. 2026 (PR #48)** : `SignalCard` partagée (densité `compact` : colonne sévérité + paire/temps/valeur), live SSE via `useSignalStream` réutilisé, à côté du top pools (grille 2 col `xl`).
- 🚫 **Mode clair / sombre** — **différé (décision 6 juil. 2026)**, pas supprimé. Raisons : (1) l'audience DeFi vit en dark-first (Meteora, Jupiter, Birdeye, DexScreener — c'est la convention du genre, pas un choix esthétique) ; (2) ce n'est pas un toggle mais une **2ᵉ direction artistique** — tout le système sémantique (teintes de sévérité, glows, opacités de bordures) est calibré contre fond sombre, et le vrai coût est la taxe permanente : chaque feature future conçue/vérifiée deux fois, inacceptable en solo ; (3) zéro demande mesurable pré-release — coût certain contre besoin hypothétique. **Déclencheur de réévaluation** : demandes utilisateurs réelles post-prod, ou cas d'usage desk/B2B. Prérequis technique le jour venu : tokens sémantiques (la palette est déjà en variables CSS dans `globals.css`) — pas de refactor préventif. **Alternative retenue à coût quasi nul** : vérifier les contrastes WCAG AA du thème sombre actuel (sert 100 % des utilisateurs) — complément naturel de la passe d'échelle typo du 6 juil.
- [x] **Sidebar minimisable** — **livré 6 juil. 2026 (PR #50)** : rail 76px icônes seules (`lg+`, drawer mobile intact), toggle sur la ligne du caption « Dashboard », panel réseau réduit à sa pastille d'état. Deux écarts au plan initial, motivés : **cookie** au lieu de LocalStorage (le layout serveur le lit → bon état au premier rendu, zéro flash) ; **`title` natif** au lieu de tooltips custom (un panneau flottant sortant du rail chevauche le contenu — abandonné après essai).
- [x] **Filtres locaux sur `/signals`** — **livré 6 juil. 2026** : chips à bascule sévérité (set fermé, couleurs des cards) × détecteur (**dérivées des signaux présents** — un nouveau détecteur apparaît sans code), compteurs live, OR intra-dimension / ET inter-dimensions, sélection vide = rien de caché, état éphémère (`useState`), helper pur `filter-signals.ts` testé (6 cas). État vide filtré distinct avec bouton réinitialiser. Évolutions notées : filtres en URL (partage par lien) si le besoin apparaît ; pousser `?severity=` côté serveur le jour où le local ne suffit plus.
- [x] **Déploiement de l'`InfoPopover`** — **livré 6 juil. 2026 (PR #51)** : 4 KPIs Overview + 4 cards pool-detail (TVL/volume/fees/composition) + 5 lignes analytiques (fee tier, fee split, fee effectif, parts protocole/LP), textes en/fr vérifiés contre la sémantique réelle (`PoolAnalytics`, règles de nullité). Prop `info?` optionnelle sur `StatCard`/`KpiCard`/`InfoRow`/`PoolCompositionCard`, clé générique `shell.metricInfo`.
- [x] **Explications des détecteurs sur `/signals`** — **livré 6 juil. 2026 (PR #53)** : un ⓘ unique sur le groupe de filtres « Type » (une place, pas 50 répétitions par card), panneau listant les détecteurs **connus présents** dans le feed (un détecteur inconnu est omis plutôt qu'affiché en anglais brut). Choix rédactionnels : **aucun chiffre de seuil en dur** (config env — figer 5 %/20 % dans l'i18n dériverait silencieusement au retune, chaque card affiche déjà le seuil réellement franchi) ; le texte Flux mentionne le plancher de volume (explique pourquoi une pool quasi inactive ne signale jamais).
- [x] **PoolDetail : onglet « Alertes »** (relevé 6 juil. 2026) — **livré 7 juil. 2026** : 5ᵉ onglet (`?tab=alerts`) listant les signaux de la pool avec la `SignalCard` canonique, pagination namespacée `alerts*` (pattern swaps/liq, limit 20), état vide dédié, i18n en/fr. Support API : filtre `?pool=` sur `GET /api/signals` (paramètre d'égalité optionnel dans le SQL statique de `SignalFeed::list`, même pattern que `severity` — compile-check `query_as!` conservé), validation pubkey → 400, combinable avec `severity` (AND). Fetcher `fetchSignals` passé sur un objet de params. Choix v1 actés : pas de live SSE sur l'onglet (historique par pool ; filtrage client du flux global possible plus tard sans rework), `SignalCard` inchangée (l'auto-lien paire vers la fiche courante est inoffensif).
- [x] **En-têtes de pages moins présents** — **livré 6 juil. 2026 (PR #49)** : eyebrow supprimé, titre 20px, description dans un popover ⓘ au clic (~170px → ~60px). Au passage : composant **`InfoPopover` réutilisable** (`shared/info-popover.tsx`, clic — pas hover, accessible) = le pattern maison pour toute explication à la demande ; à réutiliser pour les définitions de KPI, les explications de détecteurs, etc.

#### RGPD / légal — avant déploiement public
- [ ] Vérifier contenu page Privacy (mentions RGPD complètes)
- [ ] Vérifier contenu page Mentions légales (SASU AWSD, éditeur, hébergeur)
- [ ] Vérifier contenu pages Terms / Support / About

#### Déploiement Scaleway — 📅 démarrage 1ʳᵉ semaine d'août 2026 (décidé 2 juil.) · ⚠️ restore testé avant le 27 août (convalescence)
- [ ] Provisionner Instance DEV1-M (`fr-par-1`, Ubuntu 24.04)
- [ ] Hardening SSH (clé uniquement, fail2ban, ufw 22/80/443)
- [ ] Installer Docker + Compose plugin
- [ ] Provisionner Managed PostgreSQL, activer TimescaleDB
- [ ] Créer bucket Object Storage `yog-backups` One Zone IA
- [ ] Migrer site AWSD (Hugo → rsync → Caddy)
- [ ] Configurer Caddy + Let's Encrypt pour yog-scope.xyz
- [ ] CI/CD : GitHub Actions → registry Scaleway → SSH deploy (`docker compose pull && up -d`)
- [ ] Tester restore pg_dump avant le 27 août (impératif avant convalescence)
- [ ] Uptime Kuma + Healthchecks.io dead man switch indexer

### Reliquats v0.1 (analyzer — non bloquants, déclenchés au besoin)
- [ ] **Overview phase 1.5** : tri par TVL (variante `metric=tvl` + colonne triable / toggle) — quand le besoin se présente
- [ ] **Overview phase 2** : crate `yog-analytic` — calcul + stockage de l'analytique matérialisée (forme TBD : `MATERIALIZED VIEW` rafraîchi vs table + worker)
- [ ] **Overview phase 2** : déclencheur — quand une requête analytique **mesurée** franchit un seuil réel (notamment à l'ouverture de l'allowlist `watched_pools` ; re-mesurer, le chiffre dev de juin 2026 n'est plus représentatif)
- [ ] **Frontend / PagePool** : système de favoris stocké en LocalStorage (sinon back pour récupérer plusieurs pools par PubKey)
- [ ] **Frontend / PagePool** : ajout colonne fee + filtre (faisabilité à confirmer)
- [ ] **Transverse** : VIEW cross-protocole au-dessus des CA — à créer au 2ᵉ protocole (DLMM/Raydium), comme la VIEW `swap_events` ; lecture mono-protocole directe en attendant
- [ ] **Transverse** : extraction d'un `StreamPoller`/handler SSE **génériques** — **au 2ᵉ flux SSE** (relevé revue PR #39 : `SignalStreamPoller` + handler sont volontairement couplés aux signaux). Le squelette mécanique (tick + `receiver_count` + watermark + unfold/keep-alive/`Lagged`) est généralisable (trait `StreamSource` : curseur + `tip()`/`delta()`) ; extraction mécanique avec 2 cas concrets sous les yeux — pas avant, une abstraction déduite d'un seul exemple encoderait les hypothèses du feed signaux (global, basse fréquence, broadcast partagé). ⚠️ Au 2ᵉ flux, **re-questionner le substrat** selon sa fréquence : swaps live = haute fréquence + filtre par pool → broadcast partagé insuffisant, LISTEN/NOTIFY redevient peut-être pertinent
- [ ] **Transverse / perf** : table `pool_analytics_hourly` matérialisée (débloquera tri TVL/Volume + filtres) — relève du crate `Yog-Analytic` ; pas encore le déclencheur (5–47 ms read-time en dev), re-mesurer à l'ouverture de `watched_pools`
- [ ] **Transverse / perf** : cache HTTP `Cache-Control: max-age=30` sur `GET /api/pools`

## Pré-v0.2 — Acquisition d'un flux RPC adapté (gate de viabilité, décidé 3 juil. 2026)

> L'extension multi-protocoles (v0.2) rend ce choix **nécessaire à la viabilité du
> projet**. Sous le free tier (~10 req/s), chaque protocole ajouté se partage le
> même budget de requêtes via l'allowlist : la *profondeur par pool* reste bonne
> (pools de l'allowlist couvertes intégralement), mais l'argument « poids marché »
> de la priorisation (Raydium #1 fees, Orca leader volume) ne paie qu'avec du
> débit réel. Multiplier les protocoles en largeur avec un débit d'échantillon,
> c'est acheter la promesse sans la marchandise.
>
> Le 🚫 de v0.1 (« dépendance structurelle à Helius ») est **levé, pas oublié** :
> il devient le premier critère de choix — provider interchangeable, pas de
> couplage structurel.

- [ ] Comparer les offres **Yellowstone gRPC (Geyser) managées** : Shyft, Triton, Helius LaserStream, QuickNode — quotas / coûts / latence / free tier (reprend l'item d'étude v0.1.1 *yog-indexer — source de données* ; l'étude peut démarrer pendant v0.1.1)
- [ ] Critère de choix n°1 : **pas de dépendance structurelle à un provider unique** — couche subscription derrière une interface, provider swappable par config
- [ ] Migration de la couche subscription de l'indexer (périmètre : `RpcListener` seul ; pipeline extraction → persistance inchangé)
- [ ] Désactivation de l'allowlist `watched_pools` → architecture protocol-centric pleine
- [ ] Re-mesurer les déclencheurs différés « à l'ouverture de l'allowlist » (perf read-time Overview, table `pool_analytics_hourly` — cf. Reliquats v0.1)
- [ ] Intégrer le budget RPC mensuel au coût d'infra (~20 € HT Scaleway + flux RPC)

---

## v0.2 — Extension multi-protocoles (découpée en v0.2.x)

> **Décision de séquençage (3 juillet 2026)** : les protocoles avant l'auth —
> acquérir (couverture) → retenir (auth/watchlists, v0.3) → monétiser (referral,
> v0.4). L'auth en premier aurait été de la rétention sans audience. Corollaire
> calendrier : l'intégration protocole est du travail *recetté* (voie 3, 3
> dispatch points), compatible avec la convalescence de septembre ; l'auth est
> sécurité-critique et attendra la pleine capacité.
>
> **Gates d'entrée de v0.2** :
> 1. **Signal Engine DAMM v2 calibré empiriquement en prod** — pas juste livré :
>    seuils flow/deviation validés sur données réelles Scaleway. Multiplier les
>    protocoles avant d'avoir validé la valeur des signaux dilue l'effort.
> 2. **Flux RPC adapté acquis** (section Pré-v0.2 ci-dessus).
>
> **Pourquoi une version par protocole** : le coût réel n'est pas le décodeur
> (event_cpi identique, recette add-protocol) mais la **sémantique domaine** —
> modèle de liquidité, prix spot, signification de l'imbalance, VIEWs de lecture
> des détecteurs. Chaque v0.2.x livre un protocole **de bout en bout, détecteurs
> compris**, pas trois décodeurs sans signaux.

### v0.2.0 — Meteora DLMM

- [ ] Décodeur event_cpi + recette add-protocol (3 dispatch points, cf. `crates/README.md`)
- [ ] Sémantique domaine **bins concentrés** (≠ x·y=k) : `PoolCurrentState`, prix spot, AMM math DLMM (`core::amm`)
- [ ] VIEW cross-protocole au-dessus des CA — le déclencheur « au 2ᵉ protocole » des Reliquats v0.1 est atteint
- [ ] Couverture détecteurs : flow imbalance + price deviation adaptés au modèle bins (VIEWs de lecture dédiées, façon migrations 023/024)
- [ ] Front : fiche pool DLMM (les champs DAMM v2-spécifiques ne s'appliquent pas tels quels)

### v0.2.1 — Raydium CLMM/CPMM

- [ ] Nouvel IDL, même modèle conceptuel CLMM — décodeur + domaine + détecteurs + front
- [ ] **Re-décision throughput avant ouverture** : mesurer le budget requêtes réel post-migration RPC (Raydium = plus gros volume Solana, 147 M$/j)

### v0.2.2 — Orca Whirlpools

- [ ] CLMM bien documenté, SDK mature — décodeur + domaine + détecteurs + front
- [ ] Au 3ᵉ protocole CLMM : évaluer la factorisation de la sémantique bins/ticks partagée DLMM/Raydium/Orca (pas avant — abstraction sous preuve de 3 cas concrets)

### Référence — priorisation multi-protocoles (analyse du 3 juil. 2026)

> Quatre axes de pondération : **fit thèse** (nourrit l'analyse de liquidity
> pools AMM vs adjacent), **coût d'intégration** (proximité avec le pattern
> event_cpi Anchor déjà en place), **valeur signal** (enrichit le Signal Engine
> ou pas), **poids marché** (TVL/volume réel, snapshot avril-mai 2026 DeFiLlama).

**Tier 1 — extension directe (même famille de données, même pattern event_cpi) → c'est le périmètre v0.2.x**

| Protocole | Fit thèse | Coût intégration | Valeur signal | Poids marché |
|---|---|---|---|---|
| Meteora DLMM | 10/10 | Faible — même écosystème Meteora | 10/10 — bins concentrés, imbalance plus riche | 1,1 Md$ TVL |
| Raydium CLMM/CPMM | 9/10 | Moyen — nouvel IDL, même modèle conceptuel | 9/10 — plus gros volume réel (147M$/j) | 2,3 Md$ TVL, #1 fees (~222M$/an) |
| Orca Whirlpools | 9/10 | Moyen — CLMM bien documenté, SDK mature | 8/10 | Leader volume 24h (162M$) |

**Tier 2 — couche transversale (prix, exécution) → pas dans v0.2.x, réévalué ensuite**

| Protocole | Fit thèse | Coût intégration | Valeur signal | Poids marché |
|---|---|---|---|---|
| Jupiter (deepen) | 7/10 | Faible — Price V3 déjà intégré | 6/10 — pas un signal de pool, génère du revenu (cf. v0.4) | 70-85% du volume agrégé |
| Pyth Network | 6/10 | Moyen — nouvel oracle, réduit la dépendance à Jupiter comme source de prix unique | 7/10 — fiabilité sub-seconde | Standard de facto Solana |

**Tier 3 — adjacent, hors thèse (à surveiller comme bruit, pas à intégrer)**

| Protocole | Fit thèse | Coût intégration | Valeur signal | Poids marché |
|---|---|---|---|---|
| Kamino Finance | 4/10 | Élevé — nouveau domaine (lending + vaults) | 5/10 — ses vaults wrappent des positions Orca/Raydium CLMM → source de faux positifs possibles sur un futur détecteur TVL drain | ~2-3 Md$ TVL, #1 Solana |
| Drift Protocol | 2/10 | Élevé — perps, virtual AMM, modèle de données différent | 3/10 | 150-400M$ TVL |

**Tier 4 — hors périmètre**

Jito, Marinade, Sanctum (liquid staking) — fit thèse quasi nul, pas de
liquidity pool events comparables à un AMM. Écarté sauf besoin futur de
tracker spécifiquement les paires SOL/LST.

> Kamino n'est pas une cible d'intégration mais un **bruit à filtrer** :
> si un futur détecteur TVL drain se déclenche sur un pool Orca/Raydium à
> cause d'un rebalancing automatique Kamino plutôt qu'un vrai signal de
> marché, c'est un faux positif à connaître avant de le construire.

---

## v0.3 — Auth (ex-v0.2, pas encore attaqué)

> Repoussée derrière l'extension protocoles (décision 3 juil. 2026) : la
> rétention (watchlists, tiers) suppose une audience que la couverture v0.2
> doit d'abord créer. L'**auth wallet Solana** de cette version est aussi le
> prérequis technique du wallet-connect de v0.4.

- [ ] Tables `users`, `sessions`, `auth_methods`
- [ ] Auth email + Argon2
- [ ] OAuth Google + GitHub
- [ ] Auth wallet Solana (signature nonce)
- [ ] Watchlist personnelle par utilisateur
- [ ] Tiers placeholders (free/solo/pro) sans billing
- [ ] Réévaluation WASM en début de v0.3

## v0.4 — Monétisation : Jupiter Referral Program

> Analysé le 3 juillet 2026. Jupiter expose un **Referral Program** on-chain
> open-source : un intégrateur peut prendre une fee (bps) sur les swaps routés
> via son intégration de l'API. Jupiter retient 20% de la fee intégrateur.
> Techniquement : `referralAccount` + `referralTokenAccount` par mint, ajout
> de `referralAccount`/`referralFee` sur `/order`, sign+send via `/execute`
> (`@jup-ag/referral-sdk`).
>
> **Le vrai coût n'est pas l'API, c'est le pivot produit** : yog-sothoth est
> aujourd'hui un outil d'observation pure (pas de wallet connecté, pas de
> signature). Toucher un centime de fee suppose d'ajouter (1) connexion
> wallet frontend, (2) UI de swap (input/output/slippage/preview/signature),
> (3) gestion d'erreurs transactionnelles — un second pilier produit, pas un
> endpoint de plus. Le (1) est livré par l'auth wallet de v0.3.
>
> **Ordre de grandeur revenu** (fee nette ~40 bps après cut Jupiter) :
> - Aujourd'hui (0 utilisateur actif) : ~0 €
> - Traction modeste (~2k€/mois de volume routé) : ~8 €/mois — négligeable
> - Traction réelle (~50-100k€/mois routé) : ~200-400 €/mois
> - Revenu qui compte (plusieurs k€/mois) : suppose ~1M€/mois de volume,
>   donc une base d'utilisateurs actifs traders bien au-delà d'un outil
>   d'analyse niche
>
> **Condition de rentabilité** : ce n'est pas une feature qui rapporte en
> soi, c'est une monétisation d'une audience déjà captée par la qualité des
> signaux. Sans un Signal Engine qui donne une raison de rester sur la UI au
> moment d'agir (« ce pool a un imbalance inhabituel, je swap maintenant »),
> le trader n'a aucune raison de ne pas aller swapper directement sur
> Jupiter.ag sans payer la fee. **Prérequis implicite : Signal Engine mature
> et utilisé, pas juste livré.**
>
> **Non exclusif** : si la pression runway monte avant la traction, des tiers
> Stripe sur les signaux (alerting premium — l'ancienne piste monétisation)
> peuvent monétiser *avant* d'avoir du volume routé. Le referral a l'avantage
> de la friction zéro, pas celui de la précocité. Les deux se cumulent.

- [ ] Wallet connect frontend (Phantom/Solflare) — s'appuie sur l'auth wallet v0.3
- [ ] UI de swap (Ultra Swap API + Referral Program)
- [ ] Setup `referralAccount` + `referralTokenAccount` par mint cible
- [ ] Déclencheur : Signal Engine mature **et utilisé** (pas juste livré) — pas de date, condition d'usage
