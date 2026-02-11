PROJECT_NAME := livetree
PREFIX       := $(HOME)/.local
BINDIR       := $(PREFIX)/bin

BIN_PATH     := target/release/$(PROJECT_NAME)
INSTALL_PATH := $(BINDIR)/$(PROJECT_NAME)

.PHONY: all build clean install uninstall

all: build

build:
	cargo build --release

clean:
	cargo clean

install:
	@if [ ! -f "$(BIN_PATH)" ]; then \
		echo "Error: $(BIN_PATH) not found. Run 'make build' first."; \
		exit 1; \
	fi
	@echo "Installing $(PROJECT_NAME) to $(INSTALL_PATH)"
	install -d "$(BINDIR)"
	install -m 0755 "$(BIN_PATH)" "$(INSTALL_PATH)"

uninstall:
	@echo "Removing $(INSTALL_PATH)"
	@rm -f "$(INSTALL_PATH)"

