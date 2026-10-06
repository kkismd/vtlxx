; Assembled with RT_TEST_PROBE in state.s. The probe observes committed state
; immediately before the fatal entry passes its status to the #273 HALT.
.setcpu "6502"
.export _main, rt_test_probe
.import rt_init, rt_push, rt_add, rt_div, rt_rem, rt_store_storage, rt_store_reg
.importzp rt_depth, rt_stack
.import serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init
.if TEST_CASE = 1
    ; Binary underflow with one pre-existing operand.
    PUSH $1234
    jsr rt_add
.elseif TEST_CASE = 2
    ; Full stack remains full, including its first and last Cells.
    .repeat 16, i
        PUSH i
    .endrepeat
    PUSH 16
.elseif TEST_CASE = 3
    PUSH -7
    PUSH 0
    jsr rt_div
.elseif TEST_CASE = 4
    PUSH -7
    PUSH 0
    jsr rt_rem
.elseif TEST_CASE = 5
    ; StoreStorage cannot consume its one available operand.
    PUSH 42
    jsr rt_store_storage
.elseif TEST_CASE = 6
    ; StoreReg cannot modify a register with an empty value stack.
    ldx #25
    jsr rt_store_reg
.else
    .error "unknown probe case"
.endif
    lda #0
    jmp halt

rt_test_probe:
.if TEST_CASE = 1
    lda rt_depth
    cmp #1
    bne @bad
    lda rt_stack
    cmp #$34
    bne @bad
    lda rt_stack+1
    cmp #$12
    bne @bad
.elseif TEST_CASE = 2
    lda rt_depth
    cmp #16
    bne @bad
    ldx #0
@cell:
    txa
    lsr a
    cmp rt_stack,x
    bne @bad
    lda rt_stack+1,x
    bne @bad
    inx
    inx
    cpx #32
    bne @cell
.elseif TEST_CASE = 3 .or TEST_CASE = 4
    lda rt_depth
    cmp #2
    bne @bad
    lda rt_stack
    cmp #$f9
    bne @bad
    lda rt_stack+1
    cmp #$ff
    bne @bad
    lda rt_stack+2
    bne @bad
    lda rt_stack+3
    bne @bad
.elseif TEST_CASE = 5
    lda rt_depth
    cmp #1
    bne @bad
    lda rt_stack
    cmp #42
    bne @bad
    lda rt_stack+1
    bne @bad
.elseif TEST_CASE = 6
    lda rt_depth
    bne @bad
.endif
    lda #'K'
    jsr serial_out
    rts
@bad:
    lda #'F'
    jsr serial_out
    rts
