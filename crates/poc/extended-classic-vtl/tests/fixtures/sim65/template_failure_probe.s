.setcpu "6502"
.export _main
.import fe_compile_run, template_lookup, serial_in, serial_out, halt
.importzp fe_compile_status, cc_arg, cc_data_length

.segment "RODATA"
expected_source: .byte "A={}"

.segment "ZEROPAGE"
probe_index: .res 1

.segment "CODE"
_main:
    jsr fe_compile_run
    bcc @failed
    lda fe_compile_status
    cmp #2
    bne @failed
    lda #'a'
    jsr template_lookup
    bcs @failed
    lda cc_data_length
    cmp #4
    bne @failed
    lda cc_data_length+1
    bne @failed
    ldy #0
@compare:
    lda (cc_arg),y
    cmp expected_source,y
    bne @failed
    iny
    cpy #4
    bne @compare
    lda #'P'
    jsr serial_out
    lda #0
    jmp halt
@failed:
    lda #9
    jmp halt
