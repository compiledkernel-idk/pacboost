# pacboost Makefile

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin

.PHONY: all build install clean

all: build

build:
	@echo ":: building pacboost..."
	@cargo build --release --locked

install: build
	@echo ":: installing to $(BINDIR)..."
	@install -Dm755 target/release/pacboost $(DESTDIR)$(BINDIR)/pacboost
	@echo ":: installation complete."

clean:
	@cargo clean
