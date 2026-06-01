# Yog-Sothoth — Backlog

> Source of truth opérationnelle. Mettre à jour en fin de session / fin de journée.
> Statuts : `[ ]` à faire · `[~]` en cours · `[x]` fait · `[-]` abandonné (raison entre parenthèses)

---

## Déploiement Scaleway (v0.1 — bloquant release)

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

## Indexer — Cercle 2 events

- [ ] `EvtInitializePool` — débloque `fee_tier` dans `PoolResponse`
- [ ] `EvtCreatePosition`
- [ ] `EvtClosePosition`
- [ ] `EvtSetPoolStatus`
- [ ] `EvtUpdatePoolFees`
- [ ] Activer `fee_tier` dans `PoolResponse` une fois `EvtInitializePool` indexé

---

## Dashboard — page Overview

- [ ] Cadrage produit : définir quelles agrégations afficher (KPIs globaux ? top pools ? flux récent ?)
- [ ] Implémentation une fois le cadrage acté

---

## yog-context — hardening

- [ ] Métriques Prometheus sur worker tick metadata (10s)
- [ ] Métriques Prometheus sur worker tick price (30s)
- [ ] Worker respawn logic (actuellement abandon permanent après épuisement retry budget)

---

## yog-api — refacto application layer (pattern PoolService)

- [ ] `SwapService` avec tests unitaires (pattern identique à `PoolService`)
- [ ] `LiquidityService` avec tests unitaires

---

## yog-api — tracing HTTP

- [ ] Évaluer filtrage `/healthz` via `EnvFilter` (quand ça gêne vraiment, probablement après Scaleway)
- [ ] Dupliquer le pattern TraceLayer sur `yog-indexer`
- [ ] Dupliquer le pattern TraceLayer sur `yog-context`

---

## Frontend — dette technique

- [ ] Copy-to-clipboard sur l'adresse Solana du wallet `support-us` (actuellement plain text server-side)

---

## Frontend — page /pools (filtres)

- [-] Filtres TVL min / volume min (abandonné — TVL et volume sont calculés au read-time, pas matérialisés ; filtrage SQL efficace impossible sans table `pool_analytics` matérialisée)

---

## Performance — différé empirique

> N'activer que si la charge le justifie. Ne pas anticiper.

- [ ] Continuous aggregates TimescaleDB volume 24h — si ≥ 500 pools
- [ ] Table `pool_analytics_hourly` matérialisée (débloquera tri TVL/Volume + filtres) — si besoin avéré
- [ ] Cache HTTP `Cache-Control: max-age=30` sur `GET /api/pools`

---

## Infrastructure RPC — différé

- [-] Migration vers `transactionSubscribe` Helius ou Yellowstone gRPC (Shyft/Triton) — désactive l'allowlist `watched_pools`, architecture protocol-centric pleine. À faire quand throughput devient la contrainte réelle. (si mise en place dépendence structurel à Heluis)

---

## RGPD / légal — avant déploiement public

- [ ] Vérifier contenu page Privacy (mentions RGPD complètes)
- [ ] Vérifier contenu page Mentions légales (SASU AWSD, éditeur, hébergeur)
- [ ] Vérifier contenu pages Terms / Support / About

---

## v0.2 — Signal Engine (pas encore attaqué)

- [ ] Crate `signals` dans le workspace
- [ ] Trait `SignalDetector`, struct `Signal`
- [ ] Détecteur Fee yield spike
- [ ] Détecteur TVL drain
- [ ] Détecteur Imbalance alert (selon retour utilisateur)
- [ ] Détecteur Price impact creep (selon retour utilisateur)
- [ ] Service `signal-engine` binaire
- [ ] Table `signals` TimescaleDB
- [ ] Push alertes : webhook, email (Resend/Mailgun), Telegram
- [ ] UI feed signaux dans le dashboard

---

## v0.3 — Auth (pas encore attaqué)

- [ ] Tables `users`, `sessions`, `auth_methods`
- [ ] Auth email + Argon2
- [ ] OAuth Google + GitHub
- [ ] Auth wallet Solana (signature nonce)
- [ ] Watchlist personnelle par utilisateur
- [ ] Tiers placeholders (free/solo/pro) sans billing
- [ ] Réévaluation WASM en début de v0.3