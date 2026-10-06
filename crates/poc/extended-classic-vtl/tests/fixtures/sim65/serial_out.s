.setcpu "6502"
.export _main
.import serial_out, halt

.segment "CODE"
_main:
    lda #'O'
    jsr serial_out
    lda #'K'
    jsr serial_out
    lda #$00
    jmp halt
