.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_add, rt_sub, rt_mul, rt_div, rt_rem
.import rt_print_number, serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
    PUSH 32767
    PUSH 1
    jsr rt_add
    jsr rt_print_number
    SEP
    PUSH -32768
    PUSH 1
    jsr rt_sub
    jsr rt_print_number
    SEP
    PUSH 256
    PUSH 128
    jsr rt_mul
    jsr rt_print_number
    SEP
    PUSH -7
    PUSH 3
    jsr rt_div
    jsr rt_print_number
    SEP
    PUSH 7
    PUSH -3
    jsr rt_div
    jsr rt_print_number
    SEP
    PUSH -7
    PUSH 3
    jsr rt_rem
    jsr rt_print_number
    SEP
    PUSH 7
    PUSH -3
    jsr rt_rem
    jsr rt_print_number
    SEP
    PUSH -32768
    PUSH -1
    jsr rt_div
    jsr rt_print_number
    SEP
    PUSH -32768
    PUSH -1
    jsr rt_rem
    jsr rt_print_number
    DONE
