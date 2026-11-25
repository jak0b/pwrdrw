// original code from https://apple.stackexchange.com/a/283403 by Luyer, modified by M-1

#include <stdio.h>
#include <errno.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <pthread.h>
#include <IOKit/IOKitLib.h>
#include "civetweb.h"

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

typedef struct {
  float voltage;
  float current;
  float system_total;
} readings_t;

typedef struct {
  io_connect_t ioc;
  pthread_mutex_t lock;
} metrics_state_t;

int get_value(io_connect_t ioc, uint32_t key, float *value) {
  smc_t req, resp;
  size_t sz = sizeof(resp);

  memset(&req, 0, sizeof(smc_t));
  memset(&resp, 0, sizeof(smc_t));
  req.cmd = 5;
  req.size = 4;
  req.key = key;
  if (IOConnectCallStructMethod(ioc, 2, &req, sizeof(req), &resp, &sz) != KERN_SUCCESS) {
    return -1;
  }
  *value = resp.val;
  return 0;
}

int read_power(io_connect_t ioc, readings_t *readings) {
  // SMC keys/"codes" can be found at https://logi.wiki/index.php/SMC_Sensor_Codes
  if (get_value(ioc, ('V' << 24) + ('D' << 16) + ('0' << 8) + 'R', &readings->voltage) != 0 ||
      get_value(ioc, ('I' << 24) + ('D' << 16) + ('0' << 8) + 'R', &readings->current) != 0 ||
      get_value(ioc, ('P' << 24) + ('S' << 16) + ('T' << 8) + 'R', &readings->system_total) != 0) {
    return -1;
  }
  return 0;
}

int handle_request(struct mg_connection *conn) {
  const struct mg_request_info *request = mg_get_request_info(conn);
  metrics_state_t *state = request->user_data;
  readings_t readings;
  char body[1024];
  int length;
  int result;

  if (strcmp(request->local_uri, "/metrics") != 0) {
    mg_send_http_error(conn, 404, "Not found");
    return 404;
  }
  if (strcmp(request->request_method, "GET") != 0) {
    mg_send_http_error(conn, 405, "Method not allowed");
    return 405;
  }

  pthread_mutex_lock(&state->lock);
  result = read_power(state->ioc, &readings);
  pthread_mutex_unlock(&state->lock);
  if (result != 0) {
    mg_send_http_error(conn, 503, "Unable to read AppleSMC");
    return 503;
  }

  length = snprintf(body, sizeof(body),
    "# HELP macos_power_input_voltage_volts DC input voltage in volts.\n"
    "# TYPE macos_power_input_voltage_volts gauge\n"
    "macos_power_input_voltage_volts %.6f\n"
    "# HELP macos_power_input_current_amperes DC input current in amperes.\n"
    "# TYPE macos_power_input_current_amperes gauge\n"
    "macos_power_input_current_amperes %.6f\n"
    "# HELP macos_power_input_watts DC input power in watts.\n"
    "# TYPE macos_power_input_watts gauge\n"
    "macos_power_input_watts %.6f\n"
    "# HELP macos_system_power_watts Total system power in watts.\n"
    "# TYPE macos_system_power_watts gauge\n"
    "macos_system_power_watts %.6f\n",
    readings.voltage, readings.current, readings.voltage * readings.current,
    readings.system_total);
  if (length < 0 || (size_t)length >= sizeof(body)) {
    mg_send_http_error(conn, 500, "Unable to format metrics");
    return 500;
  }

  mg_send_http_ok(conn, "text/plain; version=0.0.4; charset=utf-8", length);
  mg_write(conn, body, (size_t)length);
  return 200;
}

void print_usage(const char *program) {
  fprintf(stderr, "Usage: %s [-l] [-c] [-r milliseconds] | [-p port]\n", program);
}

int main(int argc, char *argv[]) {
  CFMutableDictionaryRef d;
  io_iterator_t it;
  io_object_t svc;
  io_connect_t ioc;
  readings_t readings;
  metrics_state_t state;
  struct mg_callbacks callbacks = {0};
  struct mg_context *server;
  const char *server_options[5];
  char listening_address[64];
  char port_string[8];
  int loop = 0;
  int clear_screen = 0;
  int interval_ms = 1000;
  int interval_set = 0;
  int metrics_port = 0;
  int option;

  while ((option = getopt(argc, argv, "clr:p:")) != -1) {
    if (option == 'l') {
      loop = 1;
    } else if (option == 'c') {
      clear_screen = 1;
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
      interval_set = 1;
    } else if (option == 'p') {
      char *end;
      long value;

      errno = 0;
      value = strtol(optarg, &end, 10);
      if (errno != 0 || *optarg == '\0' || *end != '\0' || value <= 0 || value > 65535) {
        fprintf(stderr, "Invalid port: %s (expected 1-65535)\n", optarg);
        return 1;
      }
      metrics_port = (int)value;
    } else {
      print_usage(argv[0]);
      return 1;
    }
  }

  if (optind != argc) {
    print_usage(argv[0]);
    return 1;
  }
  if (metrics_port && (loop || clear_screen || interval_set)) {
    print_usage(argv[0]);
    return 1;
  }

  d = IOServiceMatching("AppleSMC");
  if (d == NULL || IOServiceGetMatchingServices(kIOMainPortDefault, d, &it) != KERN_SUCCESS) {
    fprintf(stderr, "Unable to find AppleSMC\n");
    return 1;
  }
  svc = IOIteratorNext(it);
  IOObjectRelease(it);
  if (svc == IO_OBJECT_NULL) {
    fprintf(stderr, "AppleSMC service is unavailable\n");
    return 1;
  }
  if (IOServiceOpen(svc, mach_task_self(), 0, &ioc) != KERN_SUCCESS) {
    IOObjectRelease(svc);
    fprintf(stderr, "Unable to open AppleSMC\n");
    return 1;
  }
  IOObjectRelease(svc);

  if (metrics_port) {
    snprintf(port_string, sizeof(port_string), "%d", metrics_port);
    snprintf(listening_address, sizeof(listening_address), "0.0.0.0:%s,[::]:%s",
      port_string, port_string);
    server_options[0] = "listening_ports";
    server_options[1] = listening_address;
    server_options[2] = "num_threads";
    server_options[3] = "4";
    server_options[4] = NULL;
    state.ioc = ioc;
    pthread_mutex_init(&state.lock, NULL);
    callbacks.begin_request = handle_request;
    server = mg_start(&callbacks, &state, server_options);
    if (server == NULL) {
      fprintf(stderr, "Unable to start metrics server on port %d\n", metrics_port);
      pthread_mutex_destroy(&state.lock);
      IOServiceClose(ioc);
      return 1;
    }
    fprintf(stderr, "Serving metrics on http://localhost:%d/metrics\n", metrics_port);
    for (;;) {
      pause();
    }
  }

  do {
    if (loop && clear_screen) {
      printf("\033[H\033[2J");
    }

    if (read_power(ioc, &readings) != 0) {
      fprintf(stderr, "Unable to read AppleSMC\n");
      IOServiceClose(ioc);
      return 1;
    }

    printf("DC In: %fV %fA %fW\n", readings.voltage, readings.current,
      readings.voltage * readings.current);
    printf("System Total: %fW\n", readings.system_total);
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
