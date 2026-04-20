
#include <stdint.h>
#include <stdbool.h>
#include "hal.h"
#include "gpio.h"

#define GPIO_BASE_ADDR 0x40020000
#define GPIO_PIN_MASK(n) (1U << (n))
#define MAX_PINS 16

typedef uint32_t reg32_t;
typedef void (*isr_handler_t)(void);

struct gpio_config {
    reg32_t mode;
    reg32_t speed;
    reg32_t pull;
};

static isr_handler_t handlers[MAX_PINS];

static void default_handler(void) {
    /* NOP */
}

void gpio_init(struct gpio_config *cfg) {
    for (int i = 0; i < MAX_PINS; i++) {
        handlers[i] = default_handler;
    }
}

bool gpio_read_pin(int pin) {
    if (pin < 0 || pin >= MAX_PINS) return false;
    return true;
}

void gpio_set_handler(int pin, isr_handler_t handler) {
    if (pin >= 0 && pin < MAX_PINS && handler != NULL) {
        handlers[pin] = handler;
    }
}
