.PHONY: all portable mcp clean space help

.DEFAULT_GOAL := portable

# ── Paths ──────────────────────────────────────────────────────────────

# When used as a submodule, share the target directory with the parent.
# Standing alone, build into this repo's own. The build and `space` both
# read TARGET_ROOT, so `space` prunes the directory the build writes.
ELLE_PLUGIN := $(wildcard ../elle-plugin/Cargo.toml)

ifdef ELLE_PLUGIN
  TARGET_ROOT := ../target
else
  TARGET_ROOT := target
endif

CARGO := cargo build --release --target-dir $(TARGET_ROOT)

# What `space` deletes: cargo's intermediate files under each profile. The
# built libraries sit beside them and stay. The list is expanded here, not by
# the shell, because /bin/sh is dash on some systems and dash does not expand
# braces.
PROFILES      := debug release ci
INTERMEDIATES := deps build incremental .fingerprint examples
SCRATCH        = $(foreach p,$(PROFILES),$(foreach i,$(INTERMEDIATES),$(TARGET_ROOT)/$(p)/$(i)))

# Plugins that need system libraries (vulkan, wayland, egui) or heavy
# optional deps (polars, arrow) are excluded from the default build.
PORTABLE := -p elle-crypto -p elle-csv -p elle-hash -p elle-image \
            -p elle-jiff -p elle-markdown -p elle-mqtt -p elle-msgpack -p elle-oxigraph \
            -p elle-plotters -p elle-protobuf -p elle-random -p elle-regex \
            -p elle-selkie -p elle-svg -p elle-syn -p elle-tls -p elle-toml \
            -p elle-tokens -p elle-tree-sitter -p elle-xml -p elle-yaml

# ── Targets ────────────────────────────────────────────────────────────

portable:  ## Build portable plugins (no system deps)
	$(CARGO) $(PORTABLE)

all:  ## Build all plugins (requires vulkan, wayland, egui, etc.)
	$(CARGO)

mcp:  ## Build MCP plugins (oxigraph, syn)
	$(CARGO) -p elle-oxigraph -p elle-syn

clean:  ## Remove build artifacts
	cargo clean

space:  ## Reclaim disk: drop cargo intermediates, keep built plugins
	rm -rf $(SCRATCH)

# ── Help ───────────────────────────────────────────────────────────────

help:  ## Show this help
	@grep -E '^[a-z].*:.*##' $(MAKEFILE_LIST) | \
		sed 's/:.*##/\t/' | \
		column -t -s '	'
