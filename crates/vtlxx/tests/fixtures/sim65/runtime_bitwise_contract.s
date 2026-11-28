; Probe the committed stack at the existing underflow error hook.
.setcpu "6502"
.export _main, rt_test_probe
.import rt_init, rt_push, rt_and, rt_or, rt_xor, serial_out, halt
.importzp rt_depth, rt_stack
.include "runtime_macros.inc"

.macro OP
.if TEST_CASE = 1 .or TEST_CASE = 2 .or TEST_CASE = 7
    jsr rt_and
.elseif TEST_CASE = 3 .or TEST_CASE = 4 .or TEST_CASE = 8
    jsr rt_or
.else
    jsr rt_xor
.endif
.endmacro

.segment "CODE"
_main:
    jsr rt_init
.if TEST_CASE <= 6
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
    PUSH $aa55
    PUSH $0ff0
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
    cmp #$50
.elseif TEST_CASE = 8
    cmp #$f5
.else
    cmp #$a5
.endif
    bne bad
    lda rt_stack+3
.if TEST_CASE = 7
    cmp #$0a
.elseif TEST_CASE = 8
    cmp #$af
.else
    cmp #$a5
.endif
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
