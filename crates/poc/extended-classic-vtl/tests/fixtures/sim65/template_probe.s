.setcpu "6502"
.export _main
.import fe_compile_run, template_lookup, serial_in, serial_out, halt
.importzp fe_compile_status, cc_arg, cc_data_length

.segment "RODATA"
expected_source: .byte "A={}",10,"{}=MAX{}"
expected_source_end:

.segment "ZEROPAGE"
probe_index: .res 1

.segment "CODE"
_main:
    jsr fe_compile_run
    bcs @compile_failed
    lda #'a'
    jsr template_lookup
    bcs @lookup_failed
    lda cc_data_length
    cmp #expected_source_end-expected_source
    bne @low_length_failed
    lda cc_data_length+1
    bne @high_length_failed
    lda #0
    sta probe_index
@compare:
    ldy probe_index
    lda (cc_arg),y
    cmp expected_source,y
    bne @contents_failed
    inc probe_index
    lda probe_index
    cmp #expected_source_end-expected_source
    bne @compare
    lda #'O'
    jsr serial_out
    lda #'K'
    jsr serial_out
    jsr serial_in
    bcs @contents_failed
    jsr serial_out
    lda #0
    jmp halt
@compile_failed:
    lda fe_compile_status
    jmp halt
@lookup_failed:
    lda #6
    jmp halt
@low_length_failed:
    lda #7
    jmp halt
@high_length_failed:
    lda #8
    jmp halt
@contents_failed:
    lda #9
    jmp halt
