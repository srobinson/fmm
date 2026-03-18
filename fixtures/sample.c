/**
 * Sample C file for fmm parser validation
 * Demonstrates functions, structs, enums, typedefs, macros, and includes
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "config.h"
#include "utils/helpers.h"

#define MAX_BUFFER_SIZE 1024
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define API_VERSION "1.0.0"

/* Typedefs */
typedef int (*Callback)(void *data, int status);
typedef unsigned long HashValue;

/* Status enumeration */
enum Status {
    STATUS_OK = 0,
    STATUS_ERROR = 1,
    STATUS_PENDING = 2
};

/* Configuration structure */
struct Config {
    char *name;
    int max_retries;
    double timeout;
    enum Status status;
};

/* Result structure */
struct Result {
    int code;
    char *message;
    void *data;
};

/* Static helper - not exported */
static int validate_input(const char *input) {
    if (input == NULL) return 0;
    return strlen(input) > 0;
}

/* Another static helper */
static void log_message(const char *msg) {
    fprintf(stderr, "[LOG] %s\n", msg);
}

/* Initialize configuration - exported (pointer return via struct) */
struct Config *config_init(const char *name, int retries) {
    struct Config *cfg = (struct Config *)malloc(sizeof(struct Config));
    if (cfg == NULL) return NULL;
    cfg->name = strdup(name);
    cfg->max_retries = retries;
    cfg->timeout = 30.0;
    cfg->status = STATUS_OK;
    return cfg;
}

/* Process data with callback - exported */
int process_data(void *data, size_t len, Callback cb) {
    if (!validate_input((const char *)data)) {
        return STATUS_ERROR;
    }
    log_message("Processing data");
    return cb(data, STATUS_OK);
}

/* Free configuration - exported */
void config_free(struct Config *cfg) {
    if (cfg != NULL) {
        free(cfg->name);
        free(cfg);
    }
}

/* Transform result - exported (pointer return) */
struct Result *transform(struct Result *input) {
    if (input == NULL) return NULL;
    input->code = STATUS_OK;
    return input;
}

/* Get buffer - exported (char pointer return) */
char *get_buffer(size_t size) {
    if (size > MAX_BUFFER_SIZE) size = MAX_BUFFER_SIZE;
    return (char *)malloc(size);
}

/* Compute hash - exported */
HashValue compute_hash(const char *data, size_t len) {
    HashValue hash = 5381;
    for (size_t i = 0; i < len; i++) {
        hash = ((hash << 5) + hash) + (unsigned char)data[i];
    }
    return hash;
}
