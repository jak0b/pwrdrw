CC = clang
CFLAGS = -Wall -Wextra -O2
TARGET = power-usage

.PHONY: all clean

all: $(TARGET)

$(TARGET): main.c
	$(CC) $(CFLAGS) main.c -o $(TARGET) -framework IOKit -framework CoreFoundation

clean:
	rm -f $(TARGET)
