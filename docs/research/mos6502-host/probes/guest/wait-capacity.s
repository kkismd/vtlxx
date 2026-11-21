.setcpu "6502"
.export _main, rt_test_probe, test_input_events, test_input_end
.import rt_init, rt_push, rt_read_char
.import serial_in, serial_out, halt
.importzp rt_depth, rt_stack, rt_input_eof

.segment "RODATA"
test_input_events:
    .byte 0, 65, 0, 66
test_input_end:

.segment "CODE"
_main:
    jsr rt_init
.if TEST_CASE = 15
    .repeat 15, i
        lda #i
        ldx #0
        jsr rt_push
    .endrepeat
    jsr rt_read_char
    lda rt_depth
    cmp #16
    bne @bad
    ldx #0
@prefix:
    txa
    lsr a
    cmp rt_stack,x
    bne @bad
    lda rt_stack+1,x
    bne @bad
    inx
    inx
    cpx #30
    bne @prefix
    lda rt_stack+30
    cmp #65
    bne @bad
    lda rt_stack+31
    bne @bad
    lda script_try_calls
    cmp #1
    bne @bad
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
.else
    .repeat 16, i
        lda #i
        ldx #0
        jsr rt_push
    .endrepeat
.if TEST_CASE = 17
    lda #1
    sta rt_input_eof
.endif
    jsr rt_read_char
    lda #99
    jmp halt
.endif

@bad:
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt

rt_test_probe:
    lda rt_depth
    cmp #16
    bne @probe_bad
.if TEST_CASE = 17
    lda rt_input_eof
    cmp #1
    bne @probe_bad
.else
    lda rt_input_eof
    bne @probe_bad
.endif
    jsr serial_in
    bcs @probe_bad
    cmp #65
    bne @probe_bad
    ldx #0
@probe_prefix:
    txa
    lsr a
    cmp rt_stack,x
    bne @probe_bad
    lda rt_stack+1,x
    bne @probe_bad
    inx
    inx
    cpx #32
    bne @probe_prefix
    lda #'K'
    jsr serial_out
    rts
@probe_bad:
    lda #'F'
    jsr serial_out
    rts
