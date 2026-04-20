
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "module.h"

#define MODULE_NAME "mydriver"
#define MODULE_VERSION 2

struct device_info {
    int id;
    char name[64];
    int status;
};

enum device_state {
    DEV_INIT = 0,
    DEV_RUNNING = 1,
    DEV_STOPPED = 2
};

static int debug_level = 0;

static void log_debug(const char *msg) {
    if (debug_level > 0) {
        fprintf(stderr, "[%s] %s\n", MODULE_NAME, msg);
    }
}

int device_init(struct device_info *dev, const char *name) {
    if (dev == NULL || name == NULL) return -1;
    dev->id = 0;
    strncpy(dev->name, name, sizeof(dev->name) - 1);
    dev->status = DEV_INIT;
    log_debug("Device initialized");
    return 0;
}

int device_start(struct device_info *dev) {
    if (dev == NULL) return -1;
    dev->status = DEV_RUNNING;
    return 0;
}

void device_cleanup(struct device_info *dev) {
    if (dev != NULL) {
        dev->status = DEV_STOPPED;
        log_debug("Device cleaned up");
    }
}
