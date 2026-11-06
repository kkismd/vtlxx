.setcpu "6502"
.export _main
.import fe_init, fe_next, serial_in, serial_out, halt
.importzp fe_status

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
    ; This byte is runtime input and proves the frame reader stopped exactly.
    jsr serial_in
    jsr serial_out
    lda #0
    jmp halt
@failed:
    lda #1
    jmp halt
