.setcpu "6502"
.export _main
.import rt_init, rt_push, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
    .repeat 16, i
        PUSH i
    .endrepeat
    PUSH 16
    lda #0
    jmp halt
