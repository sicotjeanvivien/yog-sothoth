# Yog-Sothoth — Backlog

> Source of truth opérationnelle. Mettre à jour en fin de session / fin de journée.
> Statuts : `[ ]` à faire · `[~]` en cours · `[x]` ✅ fait · `[-]`  🚫 abandonné (raison entre parenthèses · 🔜 reporté ultérieurement )

---
---
## v0.1 — Analyzer + Signal Engine

> Décision (10 juin 2026) : v0.1 et v0.2 fusionnées. Pas de release publique tant qu'il n'y a pas de signaux à offrir — un analytics Solana sans détecteurs est un viewer d'events, pas un produit. Le découpage interne (v0.1.0 / v0.1.1) reste pour conserver l'ordre de construction.

---
### v0.1.0 — Analyzer (POC, pas de release publique)

#### Indexer — Cercle 2 events
> Fondation per-protocole en place (voir section ✅ refactor voie 3 ci-dessous). Les cinq events s'ajoutent en suivant le pattern : wire event borsh → discriminator + extractor → translator → variant `MeteoraDammV2Event` → struct domaine + repo trait → table `meteora_damm_v2_<event>_events` + VIEW → bras `MeteoraDammV2EventPersistor`.

- [x] `EvtInitializePool` — débloque `fee_tier` dans `PoolResponse`
- [x] `EvtCreatePosition`
- [x] `EvtClosePosition`
- [x] `EvtLockPosition`
- [x] `EvtPermanentLockPosition`
- [x] `EvtSetPoolStatus`
- [x] `EvtUpdatePoolFees`
- [x] Test cercle 2 avec fixture ( non fait pour le `EvtSetPoolStatus` car pas d'event sur SOLSCAN  )
- [ ] Activer `fee_tier` dans `PoolResponse` une fois `EvtInitializePool` indexé
- [ ] `MeteoraDammV2InitializePoolEvent::pool_fees_raw` à décoder pour le moment c'est un  `Vec<u8>`
- [ ] `MeteoraDammV2UpdatePoolFeesEvent::params_raw` à décoder pour le moment c'est un  `Vec<u8>`

#### Dashboard — page Overview
- [ ] Cadrage produit : définir quelles agrégations afficher (KPIs globaux ? top pools ? flux récent ?)
- [ ] Implémentation une fois le cadrage acté
#### yog-api
- [x] `health.rs` — vérifier que ce n'est qu'une liveness, pas une readiness
- [ ] `MIDDLEWARE  CORS`
- [x] ErrorResponse RFC 9457

##### ✅ tracing HTTP
- [x] filtrage `/healthz` via `EnvFilter`
- [x] filtrage `/readyz`

#### Frontend
- [ ] Copy-to-clipboard sur l'adresse Solana du wallet `support-us` (actuellement plain text server-side)
- [ ] Revoir /lib/api/schema — problème si valeur nul
	- [ ] api-error-body
	- [ ] liquidity-event
	- [ ] network-status
	- [ ] page
	- [ ] pool-center-state
	- [ ] pool
	- [ ] price
	- [ ] swap-event
	- [ ] token
- [x] suppression BFF
- [x] Ajout Client Browser
	- [x] lib/api/browser/network-status.ts — browser-side, exposes fetchNetworkStatusBrowser
- [ ] KPI - Current Pool Price

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
### Transverse v0.1

#### Continuous aggregate — volume 24h (à cadrer)
> Contexte : `volume_24h_usd` est recalculé au read-time dans `pool_analytics.rs`
> (`SUM(...) FROM swap_events WHERE timestamp > NOW() - INTERVAL '24 hours'`), soit un scan
> des swaps des dernières 24 h à chaque `GET /api/pools`. Une continuous aggregate
> TimescaleDB pré-agrégerait les montants. **Pas encore acté — questions de design à
> trancher avant tout code :**

- [ ] **Sémantique du « 24h »** : buckets horaires → fenêtre quantifiée à l'heure (24 buckets complets + bucket courant partiel) vs exactitude `NOW() - 24h` actuelle. Le glissement des chiffres affichés est-il acceptable ?
- [ ] **Le bon moment** : la latence `GET /api/pools` est-elle un problème *mesuré*, ou anticipé ? (l'item était classé « différé si ≥ 500 pools » — promouvoir = coût récurrent avant que la charge le justifie)
- [ ] **Duplication per-protocole** : la CA serait liée à `meteora_damm_v2_swap_events` ; aujourd'hui le volume passe par la VIEW cross-protocole `swap_events` (agnostique). DLMM/Raydium → une CA chacun + union read-time. Simplicité read-time échangée contre perf
- [ ] **Alternatives plus légères** : index ciblé + requête actuelle, ou table matérialisée rafraîchie par l'indexer — à comparer à la CA avant de choisir
- [ ] **Conversion USD** : montants bruts dans la CA + conversion read-time aux prix courants (sémantique identique à l'actuel) vs valorisation au prix de la transaction — décision à acter

> Une fois cadré : si CA retenue, esquisse d'implémentation = migration forward-only sur la
> hypertable (`time_bucket('1 hour', …)`, `SUM(amount_a/b)`, `COUNT(*)`), `add_continuous_aggregate_policy`,
> `GRANT SELECT` à `yog_api`, réécriture de la sous-requête volume, régen `.sqlx` + test d'intégration, bench avant/après.

#### Performance — différé empirique
> N'activer que si la charge le justifie. Ne pas anticiper.
- [ ] Table `pool_analytics_hourly` matérialisée (débloquera tri TVL/Volume + filtres) — si besoin avéré
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