#include <stdio.h>
unsigned char status, value;
void probe(void);
int main(void) { probe(); printf("%u %u\n", (unsigned)status, (unsigned)value); return 0; }
