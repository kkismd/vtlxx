#include <stdio.h>
int main(void) { int c=getchar(); if (ferror(stdin)) return 6; if (c==EOF) return 2; return c; }
