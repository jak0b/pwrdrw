CC = clang
CFLAGS = -Wall -Wextra -O2
TARGET = power-usage

.PHONY: all clean submodule

all: $(TARGET)

submodule:
	git submodule update --init vendor/civetweb

$(TARGET): main.c vendor/civetweb/include/civetweb.h $(wildcard vendor/civetweb/src/*)
	$(CC) $(CFLAGS) -DNO_SSL -Ivendor/civetweb/include main.c vendor/civetweb/src/civetweb.c -o $(TARGET) -framework IOKit -framework CoreFoundation

clean:
	rm -f $(TARGET)
