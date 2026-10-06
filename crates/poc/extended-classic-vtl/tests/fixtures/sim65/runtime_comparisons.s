.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_eq, rt_ne, rt_lt, rt_le, rt_gt, rt_ge
.import rt_print_number, serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
    PUSH -1
    PUSH -1
    jsr rt_eq
    jsr rt_print_number
    SEP
    PUSH -1
    PUSH 0
    jsr rt_eq
    jsr rt_print_number
    SEP
    PUSH -1
    PUSH 0
    jsr rt_ne
    jsr rt_print_number
    SEP
    PUSH 1
    PUSH 1
    jsr rt_ne
    jsr rt_print_number
    SEP
    PUSH -1
    PUSH 0
    jsr rt_lt
    jsr rt_print_number
    SEP
    PUSH 0
    PUSH -1
    jsr rt_lt
    jsr rt_print_number
    SEP
    PUSH 1
    PUSH 0
    jsr rt_le
    jsr rt_print_number
    SEP
    PUSH 0
    PUSH 0
    jsr rt_le
    jsr rt_print_number
    SEP
    PUSH 0
    PUSH 1
    jsr rt_gt
    jsr rt_print_number
    SEP
    PUSH 1
    PUSH 0
    jsr rt_gt
    jsr rt_print_number
    SEP
    PUSH -1
    PUSH 0
    jsr rt_ge
    jsr rt_print_number
    SEP
    PUSH 0
    PUSH 0
    jsr rt_ge
    jsr rt_print_number
    DONE
