.setcpu "6502"
.macpack longbranch
.export _main, rt_test_probe
.import rt_init, rt_push, rt_read_static, serial_out, halt
.import rt_storage
.importzp rt_depth, rt_stack, rt_lhs, rt_rhs, rt_input_eof, rt_regs

.segment "RODATA"
data:
    .byte $80
    .res 255, $42
    .byte $ff

.segment "CODE"
_main:
    jsr rt_init
    lda #<data
    sta rt_lhs
    lda #>data
    sta rt_lhs+1
    lda #<257
    sta rt_rhs
    lda #>257
    sta rt_rhs+1
    lda #1
    sta rt_input_eof
    lda #$a5
    sta rt_regs
    sta rt_storage
.if TEST_CASE <> 7
    lda #$34
    ldx #$12
    jsr rt_push
.if TEST_CASE = 1
    lda #0
    ldx #0
.elseif TEST_CASE = 2
    lda #0
    ldx #1
.elseif TEST_CASE = 3
    lda #$ff
    ldx #$ff
.elseif TEST_CASE = 4
    lda #1
    ldx #1
.elseif TEST_CASE = 5
    lda #2
    ldx #1
.elseif TEST_CASE = 6
    lda #0
    sta rt_rhs
    sta rt_rhs+1
    ldx #0
.endif
    jsr rt_push
.endif
    jsr rt_read_static
.if TEST_CASE = 1 .or TEST_CASE = 2
    lda rt_depth
    cmp #2
    jne @bad
    lda rt_stack+2
.if TEST_CASE = 1
    cmp #$80
.else
    cmp #$ff
.endif
    jne @bad
    lda rt_stack+3
    jne @bad
    jsr check_common
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
.endif
@bad:
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt

rt_test_probe:
.if TEST_CASE = 7
    lda rt_depth
    jne @probe_bad
    jsr check_state
.else
    lda rt_depth
    cmp #2
    jne @probe_bad
.if TEST_CASE = 3
    lda rt_stack+2
    cmp #$ff
    jne @probe_bad
    lda rt_stack+3
    cmp #$ff
    jne @probe_bad
.elseif TEST_CASE = 4 .or TEST_CASE = 5
    lda rt_stack+2
    cmp #TEST_CASE-3
    jne @probe_bad
    lda rt_stack+3
    cmp #1
    jne @probe_bad
.else
    lda rt_stack+2
    ora rt_stack+3
    jne @probe_bad
.endif
    jsr check_common
.endif
    lda #'K'
    jsr serial_out
    rts
@probe_bad:
    lda #'F'
    jsr serial_out
    rts

check_common:
    lda rt_stack
    cmp #$34
    jne check_bad
    lda rt_stack+1
    cmp #$12
    jne check_bad
    jsr check_state
    rts
check_state:
    lda rt_input_eof
    cmp #1
    jne check_bad
    lda rt_regs
    cmp #$a5
    jne check_bad
    lda rt_storage
    cmp #$a5
    jne check_bad
    lda data
    cmp #$80
    jne check_bad
    lda data+256
    cmp #$ff
    jne check_bad
    rts
check_bad:
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt
