.setcpu "6502"
.export _main, test_input_events, test_input_end
.import fe_init, fe_next, serial_in, serial_out, halt
.importzp fe_status

.segment "DATA"
; Length 2, source bytes NUL and FF, then runtime sentinel Z.
test_input_events:
    .byte 0,2, 0,0, 0,0, 0,255, 0,90, 2,0
test_input_end:

.segment "CODE"
_main:
    jsr fe_init
    bcs @failed
@source:
    jsr fe_next
    bcs @source_done
    jsr serial_out
    jmp @source
@source_done:
    lda fe_status
    bne @failed
    jsr serial_in
    bcs @failed
    jsr serial_out
    lda #0
    jmp halt
@failed:
    lda #1
    jmp halt
