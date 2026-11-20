.setcpu "6502"
.export _main, test_input_events, test_input_end
.import serial_try_in, serial_in, halt

.segment "DATA"
test_input_events:
    .byte 3,0
test_input_end:

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
