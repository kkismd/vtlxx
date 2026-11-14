; Assembled with RT_TEST_PROBE in state.s. The probe observes committed state
; immediately before the fatal entry passes its status to the #273 HALT.
.setcpu "6502"
.export _main, rt_test_probe
.import rt_init, rt_push, rt_add, rt_div, rt_rem, rt_store_storage, rt_store_reg, rt_read_char, rt_read_entropy
.import entropy_test_calls
.importzp rt_depth, rt_stack, rt_regs
.import rt_storage
.import rt_rng_state
.import serial_in, serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
.if TEST_CASE = 11 .or TEST_CASE = 12
    lda #0
    sta entropy_test_calls
.endif
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
.elseif TEST_CASE = 7
    .repeat 16, i
        PUSH i
    .endrepeat
    jsr rt_read_char
.elseif TEST_CASE = 8
    .repeat 16, i
        PUSH i
    .endrepeat
    lda #$a5
    sta rt_regs
    sta rt_regs+2
    sta rt_storage
    sta rt_storage+2
    lda #$5a
    sta rt_regs+1
    sta rt_regs+3
    sta rt_storage+1
    sta rt_storage+3
    lda #0
    sta entropy_test_calls
    jsr rt_read_entropy
.elseif TEST_CASE = 9
    PUSH $1234
    lda #$a5
    sta rt_regs
    sta rt_regs+2
    sta rt_storage
    sta rt_storage+2
    lda #$5a
    sta rt_regs+1
    sta rt_regs+3
    sta rt_storage+1
    sta rt_storage+3
    lda #0
    sta entropy_test_calls
    jsr rt_read_entropy
.elseif TEST_CASE = 10
    .repeat 16, i
        PUSH i
    .endrepeat
    lda #$a5
    sta rt_regs
    sta rt_storage
    lda #$80
    sta rt_rng_state
    lda #$81
    sta rt_rng_state+1
    lda #0
    sta entropy_test_calls
    jsr rt_read_entropy
.elseif TEST_CASE = 11
    PUSH $1234
    lda #$a5
    sta rt_regs
    sta rt_storage
    jsr rt_read_entropy
    jsr rt_read_entropy
    jsr rt_test_probe
.elseif TEST_CASE = 12
    jsr rt_read_entropy
    jsr rt_init
    jsr rt_read_entropy
    jsr rt_test_probe
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
.elseif TEST_CASE = 7
    lda rt_depth
    cmp #16
    bne @bad
    ldx #0
@input_stack:
    txa
    lsr a
    cmp rt_stack,x
    bne @bad
    lda rt_stack+1,x
    bne @bad
    inx
    inx
    cpx #32
    bne @input_stack
    jsr serial_in
    bcs @bad
    cmp #'Z'
    bne @bad
.elseif TEST_CASE = 8
    lda rt_depth
    cmp #16
    bne @bad
    lda entropy_test_calls
    bne @bad
    lda rt_regs
    cmp #$a5
    bne @bad
    lda rt_storage
    cmp #$a5
    bne @bad
    lda rt_regs+2
    cmp #$a5
    bne @bad
    lda rt_storage+2
    cmp #$a5
    bne @bad
    lda rt_regs+1
    cmp #$5a
    bne @bad
    lda rt_regs+3
    cmp #$5a
    bne @bad
    lda rt_storage+1
    cmp #$5a
    bne @bad
    lda rt_storage+3
    cmp #$5a
    bne @bad
    ldx #0
@full:
    txa
    lsr a
    cmp rt_stack,x
    bne @bad
    lda rt_stack+1,x
    bne @bad
    inx
    inx
    cpx #32
    bne @full
.elseif TEST_CASE = 9
    lda rt_depth
    cmp #1
    bne @bad
    lda rt_stack
    cmp #$34
    bne @bad
    lda rt_stack+1
    cmp #$12
    bne @bad
    lda entropy_test_calls
    cmp #1
    bne @bad
    lda rt_rng_state
    ora rt_rng_state+1
    bne @bad
    lda rt_regs
    cmp #$a5
    bne @bad
    lda rt_storage
    cmp #$a5
    bne @bad
    lda rt_regs+2
    cmp #$a5
    bne @bad
    lda rt_storage+2
    cmp #$a5
    bne @bad
    lda rt_regs+1
    cmp #$5a
    bne @bad
    lda rt_regs+3
    cmp #$5a
    bne @bad
    lda rt_storage+1
    cmp #$5a
    bne @bad
    lda rt_storage+3
    cmp #$5a
    bne @bad
.elseif TEST_CASE = 10
    lda rt_depth
    cmp #16
    bne @bad
    lda entropy_test_calls
    bne @bad
    lda rt_rng_state
    cmp #$80
    bne @bad
    lda rt_rng_state+1
    cmp #$81
    bne @bad
    lda rt_regs
    cmp #$a5
    bne @bad
    lda rt_storage
    cmp #$a5
    bne @bad
    ldx #0
@full_initialized:
    txa
    lsr a
    cmp rt_stack,x
    bne @bad
    lda rt_stack+1,x
    bne @bad
    inx
    inx
    cpx #32
    bne @full_initialized
.elseif TEST_CASE = 11
    lda rt_depth
    cmp #3
    bne @bad
    lda entropy_test_calls
    cmp #1
    bne @bad
    lda rt_stack
    cmp #$34
    bne @bad
    lda rt_stack+1
    cmp #$12
    bne @bad
    lda rt_stack+2
    cmp #$c0
    bne @bad
    lda rt_stack+3
    cmp #$40
    bne @bad
    lda rt_stack+4
    cmp #$10
    bne @bad
    lda rt_stack+5
    cmp #$30
    bne @bad
    lda rt_rng_state
    cmp #$21
    bne @bad
    lda rt_rng_state+1
    cmp #$60
    bne @bad
    lda rt_regs
    cmp #$a5
    bne @bad
    lda rt_storage
    cmp #$a5
    bne @bad
.elseif TEST_CASE = 12
    lda rt_depth
    cmp #1
    bne @bad
    lda rt_stack
    cmp #$c0
    bne @bad
    lda rt_stack+1
    cmp #$40
    bne @bad
    lda entropy_test_calls
    cmp #2
    bne @bad
    lda rt_rng_state
    cmp #$81
    bne @bad
    lda rt_rng_state+1
    cmp #$81
    bne @bad
.endif
    lda #'K'
    jsr serial_out
    rts
@bad:
    lda #'F'
    jsr serial_out
    rts
