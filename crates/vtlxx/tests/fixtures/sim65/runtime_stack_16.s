.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_print_number, serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
    .repeat 16, i
        PUSH i
    .endrepeat
    .repeat 16, i
        jsr rt_print_number
        .if i < 15
            SEP
        .endif
    .endrepeat
    DONE
