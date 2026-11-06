.setcpu "6502"
.export _main
.import rt_init, rt_print_number, halt

.segment "CODE"
_main:
    jsr rt_init
    jsr rt_print_number
    lda #0
    jmp halt
