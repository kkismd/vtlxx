.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_div, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
    PUSH -7
    PUSH 0
    jsr rt_div
    lda #0
    jmp halt
