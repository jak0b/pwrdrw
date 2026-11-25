CC = clang
CFLAGS = -Wall -Wextra -O2
TARGET = power-usage
PORT ?= 9100

.PHONY: all clean install uninstall submodule

all: $(TARGET)

submodule:
	git submodule update --init vendor/civetweb

install: $(TARGET)
	PORT="$(PORT)" ./scripts/install.sh "$(TARGET)"

uninstall:
	./scripts/uninstall.sh

$(TARGET): main.c vendor/civetweb/include/civetweb.h $(wildcard vendor/civetweb/src/*)
	$(CC) $(CFLAGS) -DNO_SSL -Ivendor/civetweb/include main.c vendor/civetweb/src/civetweb.c -o $(TARGET) -framework IOKit -framework CoreFoundation

clean:
	rm -f $(TARGET)
