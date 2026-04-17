.PHONY: all mcp clean help

.DEFAULT_GOAL := mcp

# ── Paths ──────────────────────────────────────────────────────────────

# When used as a submodule, elle-plugin lives one level up.
ELLE_PLUGIN := $(wildcard ../elle-plugin/Cargo.toml)

ifdef ELLE_PLUGIN
  PATCH := --config 'patch."https://github.com/elle-lisp/elle".elle-plugin.path="../elle-plugin"'
  TARGET_DIR := --target-dir ../target
else
  PATCH :=
  TARGET_DIR :=
endif

# ── Targets ────────────────────────────────────────────────────────────

all:  ## Build all plugins
	cargo build --release $(PATCH) $(TARGET_DIR)

mcp:  ## Build MCP plugins (oxigraph, syn)
	cargo build --release -p elle-oxigraph -p elle-syn $(PATCH) $(TARGET_DIR)

clean:  ## Remove build artifacts
	cargo clean

# ── Help ───────────────────────────────────────────────────────────────

help:  ## Show this help
	@grep -E '^[a-z].*:.*##' $(MAKEFILE_LIST) | \
		sed 's/:.*##/\t/' | \
		column -t -s '	'
