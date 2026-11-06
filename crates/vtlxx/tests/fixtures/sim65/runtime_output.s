.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_print_number, rt_print_char
.importzp rt_depth
.import serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
    NUMBER 0
    SEP
    NUMBER -12
    SEP
    NUMBER 32767
    SEP
    NUMBER -32768
    SEP
    PUSH $0141
    jsr rt_print_char
    lda rt_depth
    beq @empty
    lda #9
    jmp halt
@empty:
    DONE
