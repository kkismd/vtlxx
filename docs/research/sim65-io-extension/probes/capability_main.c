#include <stdio.h>
unsigned char cap_a, cap_x, cap_y;
void capcheck(void);
int main(void) {
    capcheck();
    if (cap_a != 0x56 || cap_x != 0x54 || cap_y != 1) return 7;
    puts("VTLXX-SIM65-TRY-ABI-1");
    return 0;
}
