# lazybones app/ — developer tasks for the React Native shell + its browser preview.
#
# The app is bare React Native (RN 0.86 + Re.Pack, NOT Expo) with the shared @nube/app-sdk client.
# There are two ways to RUN it:
#   web       the react-native-web PREVIEW — the SAME App.tsx + screens in a browser on this PC,
#             no emulator (react-native-web via vite.config.web.mts). The fast UI-iteration loop.
#   android   / ios — a real device build (needs the Android SDK / Xcode; see shell/README.md).
#
# The preview talks to a REAL gateway node (the `test_gateway` bin), exactly like the app's tests.
# TL;DR:
#   make setup     one-time: standalone pnpm install in shell/ + sdk/
#   make gateway   run the real gateway node on GW_PORT (the preview points at it)
#   make web       run the browser preview on WEB_PORT
#   make dev       gateway + web preview together (Ctrl-C stops both)
#   make kill      free the preview ports if a previous run was left running
#   make test      the sdk's real-gateway suite (spawns its own node)

HERE     := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
REPO     := $(abspath $(HERE)..)
RUST     := $(REPO)/rust
SHELL_DIR := $(HERE)shell
SDK_DIR  := $(HERE)sdk

# The gateway the preview points at, and the vite preview server port (must match
# vite.config.web.mts `server.port`). Override on the command line if they collide.
# GW_PORT is deliberately NOT 8080: that's the root Makefile's dev node (persistent store,
# possibly owned by another terminal). The preview gets its own throwaway node on its own port.
GW_HOST  ?= 127.0.0.1
GW_PORT  ?= 8087
GW_URL   := http://$(GW_HOST):$(GW_PORT)
WEB_PORT ?= 5310
WEB_URL  := http://127.0.0.1:$(WEB_PORT)

# The real gateway harness binary (same one app/sdk tests spawn). Built on demand by `gateway`.
GW_BIN   := $(RUST)/target/debug/test_gateway

.PHONY: setup gateway web dev android ios test typecheck kill

## one-time: standalone installs (shell/ is deliberately OUTSIDE the pnpm workspace — see .npmrc).
setup:
	cd $(SDK_DIR) && pnpm install
	cd $(SHELL_DIR) && pnpm install --ignore-workspace

## build (if needed) and run the REAL gateway node the preview/tests talk to. The store is
## IN-MEMORY (test_gateway sets no LB_STORE_PATH → Store::memory in host boot), so every boot is
## fresh by construction and the prefilled dev login (ada/acme) always bootstraps cleanly — a
## workspace is owned by whoever logs in FIRST (global-identity decision #4). If ada/acme ever
## 403s "not a member", the cause is a STALE test_gateway still holding $(GW_PORT) (the new one
## fails to bind and exits, so you keep talking to the old node's memory) — which is why we free
## the port first. LB_DIR below only holds node keys/config, not the store.
gateway:
	cd $(RUST) && cargo build -p lb-role-gateway --features test-harness --bin test_gateway
	@pid=$$(fuser $(GW_PORT)/tcp 2>/dev/null | tr -d ' '); \
	if [ -n "$$pid" ]; then \
		exe=$$(readlink /proc/$$pid/exe 2>/dev/null); \
		case "$$exe" in \
		*test_gateway) \
			kill -TERM $$pid 2>/dev/null; i=0; \
			while kill -0 $$pid 2>/dev/null; do \
				i=$$((i+1)); \
				if [ $$i -ge 50 ]; then kill -KILL $$pid 2>/dev/null; break; fi; \
				sleep 0.1; \
			done ;; \
		*) \
			echo "ERROR: port $(GW_PORT) is held by PID $$pid ($$exe) — NOT a test_gateway."; \
			echo "That's someone else's node (e.g. the root 'make dev' node). Refusing to kill it."; \
			echo "Run with another port: make gateway GW_PORT=<free-port>"; \
			exit 1 ;; \
		esac; \
	fi
	PORT=$(GW_PORT) \
	LB_DEVKIT_ROOT=$(RUST)/extensions \
	LB_DIR=$(RUST)/target/app-preview-lb \
	$(GW_BIN)

## the react-native-web preview (browser). Point a browser at $(WEB_URL)/?node=$(GW_URL).
## NOTE: HMR reflects edits live, but ADDING/REMOVING a module export (vite caches a `?t=` copy)
## can serve a stale module → "does not provide an export named X" + a blank root. Restart `make web`
## (or `rm -rf shell/node_modules/.vite`) after an export change; HMR alone won't pick it up.
web:
	@echo "preview → $(WEB_URL)/?node=$(GW_URL)"
	cd $(SHELL_DIR) && pnpm web

## gateway + web preview together; Ctrl-C stops both (kills the whole process group on exit).
dev:
	@$(MAKE) -C $(HERE:%/=%) --no-print-directory _dev

_dev:
	@echo "starting gateway ($(GW_URL)) + web preview ($(WEB_URL)) — Ctrl-C stops both"
	@trap '$(MAKE) -f $(HERE)Makefile kill' INT TERM EXIT; \
	( $(MAKE) -f $(HERE)Makefile gateway & \
	  $(MAKE) -f $(HERE)Makefile web & \
	  wait )

## real device builds (need the native toolchain — see shell/README.md).
android:
	cd $(SHELL_DIR) && pnpm android
ios:
	cd $(SHELL_DIR) && pnpm ios

## the sdk's real-gateway test suite (spawns its own throwaway node; no `make gateway` needed).
test:
	cd $(SDK_DIR) && pnpm test:gateway

## typecheck both packages.
typecheck:
	cd $(SDK_DIR) && pnpm typecheck
	cd $(SHELL_DIR) && pnpm typecheck

# Free the preview ports and reap any orphaned gateway/vite. Mirrors the root Makefile's `kill`:
# SIGTERM first, then poll pgrep and escalate stragglers to SIGKILL after ~5s. The wait is both the
# graceful-shutdown grace period AND the race-closer so a returning `make kill` immediately followed
# by `make dev` doesn't fight the dying process for the port. We match the test_gateway binary and
# ONLY the app-shell vite (vite.config.web.mts) by pattern — a bare '[v]ite' would also kill the
# web shell's ui/ vite from another terminal. (Bracketed first char so pgrep/pkill don't match
# themselves.)
kill:
	-@fuser -TERM -k $(GW_PORT)/tcp 2>/dev/null || true
	-@fuser -TERM -k $(WEB_PORT)/tcp 2>/dev/null || true
	-@pkill -TERM -f '[t]est_gateway' 2>/dev/null || true
	-@pkill -TERM -f '[v]ite.*vite.config.web.mts' 2>/dev/null || true
	@i=0; \
	while pgrep -f '[t]est_gateway' >/dev/null 2>&1 \
	   || pgrep -f '[v]ite.*vite.config.web.mts' >/dev/null 2>&1; do \
		i=$$((i+1)); \
		if [ $$i -ge 50 ]; then \
			pkill -KILL -f '[t]est_gateway' 2>/dev/null || true; \
			pkill -KILL -f '[v]ite.*vite.config.web.mts' 2>/dev/null || true; \
			break; \
		fi; \
		sleep 0.1; \
	done
	@echo "freed ports $(GW_PORT)/$(WEB_PORT) and killed any orphaned test_gateway/vite"
