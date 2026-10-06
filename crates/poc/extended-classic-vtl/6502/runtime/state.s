; EC07 target-private runtime state. Cells are little-endian signed 16-bit words.
.setcpu "6502"

.export rt_init, rt_push, rt_load_reg, rt_store_reg
.export rt_load_storage, rt_store_storage
.export rt_need_one, rt_need_two, rt_commit_binary
.export rt_underflow, rt_overflow, rt_div_zero, rt_rem_zero
.exportzp rt_depth, rt_stack, rt_lhs, rt_rhs, rt_result
.import halt
.ifdef RT_TEST_PROBE
.import rt_test_probe
.endif

.segment "ZEROPAGE"
rt_depth:   .res 1
rt_stack:   .res 32
rt_regs:    .res 52
rt_lhs:     .res 2
rt_rhs:     .res 2
rt_result:  .res 2
rt_index:   .res 1

.segment "BSS"
rt_storage: .res 256

.segment "CODE"
rt_init:
    lda #0
    sta rt_depth
    ldx #51
@clear_regs:
    sta rt_regs,x
    dex
    bpl @clear_regs
    tax
@clear_storage:
    sta rt_storage,x
    inx
    bne @clear_storage
    rts

; Internal stack construction used by the later native emitter: A low, X high.
rt_push:
    ldy rt_depth
    cpy #16
    bcc @space
    jmp rt_overflow
@space:
    pha
    txa
    pha
    tya
    asl a
    tay
    pla
    sta rt_stack+1,y
    pla
    sta rt_stack,y
    inc rt_depth
    rts

rt_need_one:
    lda rt_depth
    bne @present
    jmp rt_underflow
@present:
    rts

; Keep the committed stack untouched until a helper has computed its result.
rt_need_two:
    lda rt_depth
    cmp #2
    bcs @present
    jmp rt_underflow
@present:
    sec
    sbc #2
    asl a
    tay
    lda rt_stack,y
    sta rt_lhs
    lda rt_stack+1,y
    sta rt_lhs+1
    iny
    iny
    lda rt_stack,y
    sta rt_rhs
    lda rt_stack+1,y
    sta rt_rhs+1
    rts

rt_commit_binary:
    lda rt_depth
    sec
    sbc #2
    asl a
    tay
    lda rt_result
    sta rt_stack,y
    lda rt_result+1
    sta rt_stack+1,y
    dec rt_depth
    rts

; X is a known register number in 0..25, selected by generated code.
rt_load_reg:
    txa
    asl a
    tax
    lda rt_regs,x
    pha
    lda rt_regs+1,x
    tax
    pla
    jmp rt_push

rt_store_reg:
    stx rt_index
    jsr rt_need_one
    lda rt_index
    asl a
    tax
    lda rt_depth
    sec
    sbc #1
    asl a
    tay
    lda rt_stack,y
    sta rt_regs,x
    lda rt_stack+1,y
    sta rt_regs+1,x
    dec rt_depth
    rts

; The low seven address bits select one of 128 Cells.
rt_load_storage:
    jsr rt_need_one
    lda rt_depth
    sec
    sbc #1
    asl a
    tay
    lda rt_stack,y
    and #$7f
    asl a
    tax
    lda rt_storage,x
    sta rt_stack,y
    lda rt_storage+1,x
    sta rt_stack+1,y
    rts

rt_store_storage:
    jsr rt_need_two
    lda rt_lhs
    and #$7f
    asl a
    tax
    lda rt_rhs
    sta rt_storage,x
    lda rt_rhs+1
    sta rt_storage+1,x
    dec rt_depth
    dec rt_depth
    rts

rt_underflow:
.ifdef RT_TEST_PROBE
    jsr rt_test_probe
.endif
    lda #1
    jmp halt
rt_overflow:
.ifdef RT_TEST_PROBE
    jsr rt_test_probe
.endif
    lda #2
    jmp halt
rt_div_zero:
.ifdef RT_TEST_PROBE
    jsr rt_test_probe
.endif
    lda #3
    jmp halt
rt_rem_zero:
.ifdef RT_TEST_PROBE
    jsr rt_test_probe
.endif
    lda #4
    jmp halt
