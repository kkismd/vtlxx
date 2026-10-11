.setcpu "6502"
.export _main
.import fe_compile_run, template_lookup, serial_out, halt
.importzp cc_arg, cc_data_length

.segment "ZEROPAGE"
probe_index: .res 1

.segment "CODE"
_main:
    jsr fe_compile_run
    bcs @failed
    lda #'a'
    jsr template_lookup
    bcs @failed
    lda cc_data_length
    cmp #48
    bne @failed
    lda cc_data_length+1
    cmp #1
    bne @failed
    ldy #0
    lda (cc_arg),y
    cmp #'A'
    bne @failed
    lda cc_data_length
    sec
    sbc #1
    tay
    lda (cc_arg),y
    cmp #'X'
    bne @failed
    lda #'L'
    jsr serial_out
    lda #0
    jmp halt
@failed:
    lda #9
    jmp halt
