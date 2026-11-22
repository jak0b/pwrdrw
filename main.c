// Build with: gcc psudata.cc -o psudata -framework IOKit

// original code from https://apple.stackexchange.com/a/283403 by Luyer, modified by M-1

#include <stdio.h>
#include <errno.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <IOKit/IOKitLib.h>

typedef struct {
  uint32_t key;
  char ignored[24];
  uint32_t size;
  char ignored2[10];
  char cmd;
  char ignored3[5];
  float val;
  char ignored4[28];
} smc_t;

float get_value(io_connect_t * ioc, uint32_t key) {
  smc_t req, resp;
  size_t sz = sizeof(resp);

  memset(&req, 0, sizeof(smc_t));
  memset(&resp, 0, sizeof(smc_t));
  req.cmd = 5;
  req.size = 4;
  req.key = key;
  IOConnectCallStructMethod(*ioc, 2, &req, sizeof(req), &resp, &sz);
  return resp.val;
}

int main(int argc, char *argv[]) {
  CFMutableDictionaryRef d;
  io_iterator_t it;
  io_object_t svc;
  io_connect_t ioc;
  float v, i, system_total;
  int loop = 0;
  int interval_ms = 1000;
  int option;

  while ((option = getopt(argc, argv, "lr:")) != -1) {
    if (option == 'l') {
      loop = 1;
    } else if (option == 'r') {
      char *end;
      long value;

      errno = 0;
      value = strtol(optarg, &end, 10);
      if (errno != 0 || *optarg == '\0' || *end != '\0' || value <= 0 || value > INT_MAX) {
        fprintf(stderr, "Invalid interval: %s (expected a positive number of milliseconds)\n", optarg);
        return 1;
      }
      interval_ms = (int)value;
    } else {
      fprintf(stderr, "Usage: %s [-l] [-r milliseconds]\n", argv[0]);
      return 1;
    }
  }

  if (optind != argc) {
    fprintf(stderr, "Usage: %s [-l] [-r milliseconds]\n", argv[0]);
    return 1;
  }

  d = IOServiceMatching("AppleSMC");
  IOServiceGetMatchingServices(kIOMainPortDefault, d, &it);
  svc = IOIteratorNext(it);
  IOObjectRelease(it);
  IOServiceOpen(svc, mach_task_self(), 0, &ioc);
  IOObjectRelease(svc);

  do {
    // SMC keys/"codes" can be found at https://logi.wiki/index.php/SMC_Sensor_Codes
    v = get_value(&ioc, ('V' << 24) + ('D' << 16) + ('0' << 8) + 'R');
    i = get_value(&ioc, ('I' << 24) + ('D' << 16) + ('0' << 8) + 'R');
    system_total = get_value(&ioc, ('P' << 24) + ('S' << 16) + ('T' << 8) + 'R');

    printf("DC In: %fV %fA %fW\n", v, i, v*i);
    printf("System Total: %fW\n", system_total);
    fflush(stdout);

    if (loop) {
      struct timespec remaining = {interval_ms / 1000, (interval_ms % 1000) * 1000000L};

      while (nanosleep(&remaining, &remaining) == -1 && errno == EINTR) {
      }
    }
  } while (loop);

  IOServiceClose(ioc);
  return 0;
}
