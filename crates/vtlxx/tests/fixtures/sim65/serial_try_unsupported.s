.setcpu "6502"
.export _main
.import serial_try_in, halt

.segment "CODE"
_main:
    jsr serial_try_in
    cpx #3
    bne @fail
    lda #0
    jmp halt
@fail:
    lda #9
    jmp halt
