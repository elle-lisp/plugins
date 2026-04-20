.PHONY: all portable mcp clean help

.DEFAULT_GOAL := portable

# ── Paths ──────────────────────────────────────────────────────────────

# When used as a submodule, share the target directory with the parent.
ELLE_PLUGIN := $(wildcard ../elle-plugin/Cargo.toml)

ifdef ELLE_PLUGIN
  TARGET_DIR := --target-dir ../target
else
  TARGET_DIR :=
endif

CARGO := cargo build --release $(TARGET_DIR)

# Plugins that need system libraries (vulkan, wayland, egui) or heavy
# optional deps (polars, arrow) are excluded from the default build.
PORTABLE := -p elle-crypto -p elle-csv -p elle-hash -p elle-image \
            -p elle-jiff -p elle-mqtt -p elle-msgpack -p elle-oxigraph \
            -p elle-plotters -p elle-protobuf -p elle-random -p elle-regex \
            -p elle-selkie -p elle-svg -p elle-syn -p elle-tls -p elle-toml \
            -p elle-tree-sitter -p elle-xml -p elle-yaml

# ── Targets ────────────────────────────────────────────────────────────

portable:  ## Build portable plugins (no system deps)
	$(CARGO) $(PORTABLE)

all:  ## Build all plugins (requires vulkan, wayland, egui, etc.)
	$(CARGO)

mcp:  ## Build MCP plugins (oxigraph, syn)
	$(CARGO) -p elle-oxigraph -p elle-syn

clean:  ## Remove build artifacts
	cargo clean

# ── Help ───────────────────────────────────────────────────────────────

help:  ## Show this help
	@grep -E '^[a-z].*:.*##' $(MAKEFILE_LIST) | \
		sed 's/:.*##/\t/' | \
		column -t -s '	'
