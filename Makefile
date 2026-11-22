CC = clang
CFLAGS = -Wall -Wextra -O2
TARGET = power-usage

.PHONY: all clean

all: $(TARGET)

$(TARGET): main.c $(wildcard vendor/civetweb/*)
	$(CC) $(CFLAGS) -DNO_SSL -Ivendor/civetweb main.c vendor/civetweb/civetweb.c -o $(TARGET) -framework IOKit -framework CoreFoundation

clean:
	rm -f $(TARGET)
