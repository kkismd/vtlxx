.setcpu "6502"
.export _main, test_input_events, test_input_end
.import fe_init, halt, script_read_calls
.importzp fe_status

.segment "DATA"
test_input_events:
    .byte 0,2, 2,0              ; Low length byte, then EOF before high byte.
test_input_end:

.segment "CODE"
_main:
    jsr fe_init
    bcc @unexpected_success
    lda fe_status
    cmp #1
    bne @unexpected_success
    lda script_read_calls
    cmp #2                         ; Both header bytes were attempted; no runtime read.
    bne @unexpected_success
    lda #0
    jmp halt
@unexpected_success:
    lda #9
    jmp halt
