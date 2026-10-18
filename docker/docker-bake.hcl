# docker/docker-bake.hcl — the eight production images of a version, as the
# `images` workflow builds and pushes them (docker/README.md, "Deploying").
# One bake session: the seven backend targets share one cargo-chef builder.
# The compose files cannot stand in for it: the production overlay refuses
# to load without the server's variables.

variable "REGISTRY" {
  default = "ghcr.io/sicotjeanvivien"
}

variable "VERSION" {
  validation {
    condition     = can(regex("^v[0-9]+\\.[0-9]+\\.[0-9]+$", VERSION))
    error_message = "VERSION must be a version tag vX.Y.Z."
  }
}

# docker/pull-images.sh refuses an image whose revision is not the tag's commit.
variable "REVISION" {
  validation {
    condition     = can(regex("^[0-9a-f]{40}$", REVISION))
    error_message = "REVISION must be the full commit of the tag."
  }
}

variable "NEXT_PUBLIC_YOG_API_URL" {
  default = ""
}

# Empty: no build cache, as for a build on a workstation.
variable "CACHE" {
  default = ""
}

group "default" {
  targets = ["backend", "yog-web"]
}

target "_common" {
  platforms = ["linux/amd64"]
  labels = {
    "org.opencontainers.image.source"   = "https://github.com/sicotjeanvivien/yog-sothoth"
    "org.opencontainers.image.revision" = REVISION
    "org.opencontainers.image.version"  = VERSION
  }
}

target "backend" {
  name       = svc
  inherits   = ["_common"]
  matrix     = { svc = ["yog-migrate", "yog-indexer", "yog-api", "yog-context", "yog-signals", "yog-analytic", "yog-archive"] }
  context    = "."
  dockerfile = "docker/backend.Dockerfile"
  target     = svc
  tags       = ["${REGISTRY}/${svc}:${VERSION}"]
  cache-from = CACHE == "" ? [] : ["type=registry,ref=${CACHE}:backend"]
  # ⚠️ One exporter only: each of the seven would write the whole builder,
  # gigabytes, to the same reference.
  cache-to = CACHE == "" || svc != "yog-migrate" ? [] : ["type=registry,ref=${CACHE}:backend,mode=max"]
}

target "yog-web" {
  inherits   = ["_common"]
  context    = "web"
  dockerfile = "Dockerfile"
  args       = { NEXT_PUBLIC_YOG_API_URL = NEXT_PUBLIC_YOG_API_URL }
  tags       = ["${REGISTRY}/yog-web:${VERSION}"]
  cache-from = CACHE == "" ? [] : ["type=registry,ref=${CACHE}:web"]
  cache-to   = CACHE == "" ? [] : ["type=registry,ref=${CACHE}:web,mode=max"]
}
