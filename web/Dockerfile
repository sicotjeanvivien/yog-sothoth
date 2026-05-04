# syntax=docker/dockerfile:1.7
# ---------------------------------------------------------------------
# Yog-Sothoth web — multi-stage Dockerfile
#
# Stage 1 (deps):    install npm dependencies in isolation so that
#                    layer caching kicks in on every code-only change.
# Stage 2 (builder): compile the Next.js standalone output.
# Stage 3 (runner):  minimal Alpine image that runs the built server.
# ---------------------------------------------------------------------

# ---- Stage 1: deps ----
FROM node:22-alpine AS deps
WORKDIR /app

# Alpine is missing libc-compat, which the Next.js binary expects.
RUN apk add --no-cache libc6-compat

COPY package.json package-lock.json* ./
RUN npm ci --no-audit --no-fund

# ---- Stage 2: builder ----
FROM node:22-alpine AS builder
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY . .

# Telemetry off in CI/Docker builds — keeps logs clean and avoids
# any outbound network call during image build.
ENV NEXT_TELEMETRY_DISABLED=1

RUN npm run build

# ---- Stage 3: runner ----
FROM node:22-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

# Run as a dedicated non-root user. Keeps the container surface small
# and matches the principle of least privilege used elsewhere in the
# project (Rust indexer container, read-only PG user, etc.).
RUN addgroup --system --gid 1001 nodejs \
  && adduser --system --uid 1001 nextjs

# Copy only what is required to run the standalone server.
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static
COPY --from=builder --chown=nextjs:nodejs /app/public ./public

USER nextjs
EXPOSE 3000

CMD ["node", "server.js"]