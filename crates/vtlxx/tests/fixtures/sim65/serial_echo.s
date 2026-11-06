.setcpu "6502"
.export _main
.import serial_in, serial_out, halt

.segment "CODE"
_main:
    jsr serial_in
    jsr serial_out
    lda #$00
    jmp halt
