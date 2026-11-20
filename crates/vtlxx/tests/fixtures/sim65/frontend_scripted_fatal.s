.setcpu "6502"
.export _main, test_input_events, test_input_end
.import fe_init, halt

.segment "DATA"
test_input_events:
    .byte 3,0
test_input_end:

.segment "CODE"
_main:
    jsr fe_init
    lda #9                         ; Fatal must halt inside serial_in.
    jmp halt
