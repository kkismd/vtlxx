.setcpu "6502"
.export _main
.import serial_try_in, serial_in, serial_out, halt

.segment "CODE"
_main:
    jsr serial_try_in
    cpx #3
    bne @fail
    jsr serial_in
    bcs @fail
    cmp #'Z'
    bne @fail
    jsr serial_out
    lda #0
    jmp halt
@fail:
    lda #9
    jmp halt
