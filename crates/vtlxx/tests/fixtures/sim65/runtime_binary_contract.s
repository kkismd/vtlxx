; Verify each wrapper's failure atomicity and successful stack commit.
.setcpu "6502"
.export _main, rt_test_probe
.import rt_init, rt_push, rt_add, rt_sub, rt_mul, serial_out, halt
.importzp rt_depth, rt_stack
.include "runtime_macros.inc"

.macro OP
.if TEST_CASE = 1 .or TEST_CASE = 2 .or TEST_CASE = 7
    jsr rt_add
.elseif TEST_CASE = 3 .or TEST_CASE = 4 .or TEST_CASE = 8
    jsr rt_sub
.else
    jsr rt_mul
.endif
.endmacro

.segment "CODE"
_main:
    jsr rt_init
.if TEST_CASE <= 6
    ; Sentinel also covers the empty stack's unused first Cell.
    lda #$34
    sta rt_stack
    lda #$12
    sta rt_stack+1
.if TEST_CASE = 2 .or TEST_CASE = 4 .or TEST_CASE = 6
    PUSH $1234
.endif
    OP
    jmp bad
.else
    PUSH $1234
    PUSH 7
    PUSH 3
    OP
    lda rt_depth
    cmp #2
    bne bad
    lda rt_stack
    cmp #$34
    bne bad
    lda rt_stack+1
    cmp #$12
    bne bad
    lda rt_stack+2
.if TEST_CASE = 7
    cmp #10
.elseif TEST_CASE = 8
    cmp #4
.else
    cmp #21
.endif
    bne bad
    lda rt_stack+3
    bne bad
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
.endif

rt_test_probe:
    lda rt_depth
.if TEST_CASE = 2 .or TEST_CASE = 4 .or TEST_CASE = 6
    cmp #1
.else
    cmp #0
.endif
    bne @bad
    lda rt_stack
    cmp #$34
    bne @bad
    lda rt_stack+1
    cmp #$12
    bne @bad
    lda #'K'
    jsr serial_out
    rts
@bad:
    lda #'F'
    jsr serial_out
    rts
bad:
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt
