#include <stdint.h>

extern uint32_t sum(uint32_t, uint32_t);
extern uint32_t print_int(uint32_t);

int main(void) {
    uint32_t a = sum(1, 2);
    print_int(a);
    return 0;
}
