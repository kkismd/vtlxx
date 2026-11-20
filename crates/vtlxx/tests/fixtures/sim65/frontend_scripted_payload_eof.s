.setcpu "6502"
.export _main, test_input_events, test_input_end
.import fe_init, fe_next, serial_out, halt, script_read_calls
.importzp fe_status

.segment "DATA"
test_input_events:
    .byte 0,2, 0,0              ; Length 2.
    .byte 0,65, 2,0             ; One source byte, then EOF mid-payload.
test_input_end:

.segment "CODE"
_main:
    jsr fe_init
    bcs @unexpected_success
    jsr fe_next
    bcs @unexpected_success
    jsr serial_out
    jsr fe_next
    bcc @unexpected_success
    lda fe_status
    cmp #1
    bne @unexpected_success
    lda script_read_calls
    cmp #4                         ; Header, one payload byte, and its EOF attempt.
    bne @unexpected_success
    ; An extra call would mean the failed source phase leaked into runtime input.
    lda #0
    jmp halt
@unexpected_success:
    lda #9
    jmp halt
