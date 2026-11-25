TARGET = power-usage
PORT ?= 9100

.PHONY: all clean install uninstall

all: $(TARGET)

install: $(TARGET)
	PORT="$(PORT)" ./scripts/install.sh "$(TARGET)"

uninstall:
	./scripts/uninstall.sh

$(TARGET): Cargo.toml Cargo.lock src/main.rs
	cargo build --release
	cp target/release/$(TARGET) $(TARGET)

clean:
	rm -f $(TARGET)
	cargo clean
