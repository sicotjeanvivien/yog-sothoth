# Yog-Sothoth — Backlog

> Source of truth opérationnelle. Mettre à jour en fin de session / fin de journée.
> Statuts : `[ ]` à faire · `[~]` en cours · `[x]` ✅ fait · `[-]`  🚫 abandonné (raison entre parenthèses · 🔜 reporté ultérieurement )

---
---
## v0.1 — Analyzer + Signal Engine

> Décision (10 juin 2026) : v0.1 et v0.2 fusionnées. Pas de release publique tant qu'il n'y a pas de signaux à offrir — un analytics Solana sans détecteurs est un viewer d'events, pas un produit. Le découpage interne (v0.1.0 / v0.1.1) reste pour conserver l'ordre de construction.

---
### v0.1.0 — Analyzer (POC, pas de release publique)

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

#### Dashboard — page Overview
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
- [ ] Extension future : tri par TVL (variante `metric=tvl` + colonne triable / toggle) — quand le besoin se présente

**Phase 2 — crate `Yog-Analytic` (différé, déclenché empiriquement)**
- [ ] Crate `yog-analytic` : calcul + stockage de l'analytique matérialisée (forme TBD : `MATERIALIZED VIEW` rafraîchi vs table + worker ; cf. contraintes ci-dessus)
- [ ] Déclencheur : quand une requête analytique **mesurée** franchit un seuil réel — en particulier dès l'ouverture de l'allowlist `watched_pools` / montée du throughput cible (re-mesurer alors, le chiffre dev de juin 2026 n'est plus représentatif)

#### yog-api
- [x] `health.rs` — vérifier que ce n'est qu'une liveness, pas une readiness
- [ ] `MIDDLEWARE  CORS`
- [x] ErrorResponse RFC 9457

##### ✅ tracing HTTP
- [x] filtrage `/healthz` via `EnvFilter`
- [x] filtrage `/readyz`

#### Frontend
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

##### PagePool
- [ ] Mettre en place un systéme de favoris sur la page Pool stocker dans le LocalStorage. Je pense que c'est pas vraiment possible sinon faut du back pour pouvoir récupérer plusieurs pool via des PubKey . 
- [ ] Ajout colonne fee + filtre . Je sais pas si c'est possible . 
- [ ] Tableau liquidity — colonne « Value (USD) » : valeur USD de l'événement (amountA·prixA + amountB·prixB, valorisation trade-time comme les swaps). Chantier backend d'abord (enrichir `LiquidityEventResponse`/DTO), puis colonne front. NB : `liquidityDelta` (u128 brut, unités L sans décimales) écarté car illisible.

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

---
---

### v0.1.1 — Signal Engine (release publique)

> C'est cette phase qui justifie la mise en prod. Sans signaux, pas d'utilisateurs ; pas d'utilisateurs, pas de release.

#### Signal Engine — crate et détecteurs
- [ ] Crate `signals` dans le workspace
- [ ] Trait `SignalDetector`, struct `Signal`
- [ ] Détecteur Fee yield spike
- [ ] Détecteur TVL drain
- [ ] Détecteur Imbalance alert (selon retour utilisateur)
- [ ] Détecteur Price impact creep (selon retour utilisateur)
- [ ] Service `signal-engine` binaire
- [ ] Table `signals` TimescaleDB

#### Signal Engine — push channels
- [ ] Webhook
- [ ] Email (Resend/Mailgun)
- [ ] Telegram

#### Signal Engine — UI
- [ ] UI feed signaux dans le dashboard

#### yog-context — robustesse pour release
- [ ] Worker respawn logic (actuellement abandon permanent après épuisement retry budget)

#### Frontend — page /pools (filtres)
- [ ] Filtres TVL min / volume min — dépend de la table `pool_analytics_hourly` matérialisée (voir Transverse)

#### RGPD / légal — avant déploiement public
- [ ] Vérifier contenu page Privacy (mentions RGPD complètes)
- [ ] Vérifier contenu page Mentions légales (SASU AWSD, éditeur, hébergeur)
- [ ] Vérifier contenu pages Terms / Support / About

#### Déploiement Scaleway
- [ ] Provisionner Instance DEV1-M (`fr-par-1`, Ubuntu 24.04)
- [ ] Hardening SSH (clé uniquement, fail2ban, ufw 22/80/443)
- [ ] Installer Docker + Compose plugin
- [ ] Provisionner Managed PostgreSQL, activer TimescaleDB
- [ ] Créer bucket Object Storage `yog-backups` One Zone IA
- [ ] Migrer site AWSD (Hugo → rsync → Caddy)
- [ ] Configurer Caddy + Let's Encrypt pour yog-scope.xyz
- [ ] CI/CD : GitHub Actions → registry Scaleway → SSH deploy (`docker compose pull && up -d`)
- [ ] Tester restore pg_dump avant août (impératif avant convalescence)
- [ ] Uptime Kuma + Healthchecks.io dead man switch indexer

---
---

### Transverse v0.1

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

#### Continuous aggregates — rollups durables (cadré, 15 juin 2026)
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
- [ ] **VIEW cross-protocole au-dessus des CA** : à créer au 2ᵉ protocole (DLMM/Raydium), comme la VIEW `swap_events` actuelle — lecture mono-protocole directe en attendant

#### Performance — différé empirique
> N'activer que si la charge le justifie. Ne pas anticiper.
- [ ] Table `pool_analytics_hourly` matérialisée (débloquera tri TVL/Volume + filtres) — relèvera du crate `Yog-Analytic` (cf. Overview phase 2). Mesure 18 juin 2026 : KPIs/top-N read-time à 5–47 ms sur la DB dev → pas encore le déclencheur ; mais volume dev plafonné par `watched_pools`, à re-mesurer dès ouverture de l'allowlist
- [ ] Cache HTTP `Cache-Control: max-age=30` sur `GET /api/pools`

#### 🚫 Infrastructure RPC — différé
- [-] 🚫 Migration vers `transactionSubscribe` Helius ou Yellowstone gRPC (Shyft/Triton) — désactive l'allowlist `watched_pools`, architecture protocol-centric pleine. À faire quand throughput devient la contrainte réelle. (Non acceptable : si mise en place => dépendance structurelle à Helius)

#### 🚫 Filtres TVL/volume sans matérialisation
- [-] 🚫 Filtres TVL min / volume min sur /pools (abandonné — TVL et volume sont calculés au read-time, pas matérialisés ; filtrage SQL efficace impossible sans table `pool_analytics_hourly` matérialisée. Repris en v0.1.1 si la table est créée)

---
---

## v0.2 — Auth (ex-v0.3, pas encore attaqué)
- [ ] Tables `users`, `sessions`, `auth_methods`
- [ ] Auth email + Argon2
- [ ] OAuth Google + GitHub
- [ ] Auth wallet Solana (signature nonce)
- [ ] Watchlist personnelle par utilisateur
- [ ] Tiers placeholders (free/solo/pro) sans billing
- [ ] Réévaluation WASM en début de v0.2