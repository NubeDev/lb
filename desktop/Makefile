# desktop/Makefile — one-liner entrypoints for the Linux desktop build container.
# See desktop/docker/README.md and docs/scope/desktop/desktop-build-container-scope.md.
#
# Two invocation modes from one image (the scope's "one image, many output types" decision):
#   make linux-executable   dev: bind-mount the repo, binary lands on host (warm caches)
#   make artifact           CI : multi-stage build, extract just the ELF as a tar
#   make image              build the base toolchain image only (prereq for dev)
#   make smoke              boot the just-built binary under xvfb (liveness check)
#   make clean              remove the named cache volumes
#
# All `docker build` commands use the repo root as context (so COPY . /build in the
# builder stage and the repo-root .dockerignore both apply).

# Resolve paths regardless of where make is invoked from.
THIS_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
REPO_ROOT := $(abspath $(THIS_DIR)/..)
DOCKERFILE := $(THIS_DIR)/docker/Dockerfile
IMAGE := lazybones-shell-builder:local
BINARY_REL := ui/src-tauri/target/release/lazybones-shell
BINARY := $(REPO_ROOT)/$(BINARY_REL)
ARTIFACT := $(THIS_DIR)/lazybones-shell-linux-x86_64.tar
UID_GID := $$(id -u):$$(id -g)

DOCKER_BUILD := docker build -f $(DOCKERFILE)
DOCKER_RUN := docker run --rm --user $(UID_GID) \
  -v "$(REPO_ROOT)":/build \
  -v lb-cargo:/usr/local/cargo/registry \
  -v lb-pnpm:/usr/local/pnpm/store

.PHONY: image linux-executable artifact smoke clean help
.DEFAULT_GOAL := help

help: ## Show this help.
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*## "}{printf "  %-20s %s\n",$$1,$$2}'

image: ## Build the base toolchain image (Ubuntu 22.04 + webkit2gtk-4.1 + Rust + Node).
	$(DOCKER_BUILD) --target base -t $(IMAGE) $(REPO_ROOT)

linux-executable: image ## Dev: build lazybones-shell via the container; binary lands at $(BINARY).
	$(DOCKER_RUN) $(IMAGE)
	@echo "Binary: $(BINARY)"
	@file "$(BINARY)" 2>/dev/null || true

artifact: ## CI: build + extract just the ELF as a tar (reproducible, no bind-mount).
	$(DOCKER_BUILD) --target artifact --output type=tar,dest=$(ARTIFACT) $(REPO_ROOT)
	@echo "Artifact: $(ARTIFACT)"
	@tar -tf "$(ARTIFACT)"

smoke: ## Boot the just-built binary under xvfb in the container (liveness check).
	@test -x "$(BINARY)" || { echo "Run 'make linux-executable' first."; exit 1; }
	$(DOCKER_RUN) --entrypoint bash $(IMAGE) -c '\
	  xvfb-run -a /build/$(BINARY_REL) & SH=$$!; \
	  sleep 5; \
	  if kill -0 $$SH 2>/dev/null; then echo "alive after 5s"; kill $$SH; exit 0; \
	  else echo "DEAD within 5s"; exit 1; fi'

clean: ## Remove the cargo + pnpm named cache volumes (next build re-populates them).
	docker volume rm lb-cargo lb-pnpm 2>/dev/null || true
