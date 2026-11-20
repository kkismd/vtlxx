.setcpu "6502"
.export _main, test_input_events, test_input_end
.import serial_in, halt

.segment "DATA"
test_input_events:
    .byte 3,0
test_input_end:

.segment "CODE"
_main:
    jsr serial_in
    lda #9                         ; Fatal must not return as EOF.
    jmp halt
