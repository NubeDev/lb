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
GW_HOST  ?= 127.0.0.1
GW_PORT  ?= 8080
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

## build (if needed) and run the REAL gateway node the preview/tests talk to.
gateway:
	cargo build --manifest-path $(RUST)/Cargo.toml -p lb-role-gateway --features test-harness --bin test_gateway
	PORT=$(GW_PORT) \
	LB_DEVKIT_ROOT=$(RUST)/extensions \
	LB_DIR=$(RUST)/target/app-preview-lb \
	$(GW_BIN)

## the react-native-web preview (browser). Point a browser at $(WEB_URL)/?node=$(GW_URL).
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
# vite by pattern (bracketed first char so the pgrep/pkill don't match themselves).
kill:
	-@fuser -TERM -k $(GW_PORT)/tcp 2>/dev/null || true
	-@fuser -TERM -k $(WEB_PORT)/tcp 2>/dev/null || true
	-@pkill -TERM -f '[t]est_gateway' 2>/dev/null || true
	-@pkill -TERM -f '[v]ite' 2>/dev/null || true
	@i=0; \
	while pgrep -f '[t]est_gateway' >/dev/null 2>&1 \
	   || pgrep -f '[v]ite' >/dev/null 2>&1; do \
		i=$$((i+1)); \
		if [ $$i -ge 50 ]; then \
			pkill -KILL -f '[t]est_gateway' 2>/dev/null || true; \
			pkill -KILL -f '[v]ite' 2>/dev/null || true; \
			break; \
		fi; \
		sleep 0.1; \
	done
	@echo "freed ports $(GW_PORT)/$(WEB_PORT) and killed any orphaned test_gateway/vite"
