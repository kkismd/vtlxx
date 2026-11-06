; Signed 16-bit Cell arithmetic. All scratch space is target-private.
.setcpu "6502"

.export rt_add, rt_sub, rt_mul, rt_div, rt_rem
.export rt_eq, rt_ne, rt_lt, rt_le, rt_gt, rt_ge
.importzp rt_lhs, rt_rhs, rt_result
.import rt_need_two, rt_commit_binary, rt_div_zero, rt_rem_zero

.segment "BSS"
work_q:      .res 2             ; dividend, then quotient
work_d:      .res 2             ; unsigned divisor
work_r:      .res 3             ; 17-bit partial remainder
work_sign:   .res 1             ; bit 7: quotient sign, bit 6: remainder sign
compare_hi:  .res 2

.segment "CODE"

rt_add:
    jsr rt_need_two
    clc
    lda rt_lhs
    adc rt_rhs
    sta rt_result
    lda rt_lhs+1
    adc rt_rhs+1
    sta rt_result+1
    jmp rt_commit_binary

rt_sub:
    jsr rt_need_two
    sec
    lda rt_lhs
    sbc rt_rhs
    sta rt_result
    lda rt_lhs+1
    sbc rt_rhs+1
    sta rt_result+1
    jmp rt_commit_binary

rt_mul:
    jsr rt_need_two
    lda #$00
    sta rt_result
    sta rt_result+1
    lda rt_lhs
    sta work_q
    lda rt_lhs+1
    sta work_q+1
    lda rt_rhs
    sta work_d
    lda rt_rhs+1
    sta work_d+1
    ldx #16
@bit:
    lda work_d
    and #$01
    beq @skip_add
    clc
    lda rt_result
    adc work_q
    sta rt_result
    lda rt_result+1
    adc work_q+1
    sta rt_result+1
@skip_add:
    asl work_q
    rol work_q+1
    lsr work_d+1
    ror work_d
    dex
    bne @bit
    jmp rt_commit_binary

rt_div:
    jsr rt_need_two
    lda rt_rhs
    ora rt_rhs+1
    bne @nonzero
    jmp rt_div_zero
@nonzero:
    jsr unsigned_divide
    lda work_q
    sta rt_result
    lda work_q+1
    sta rt_result+1
    lda work_sign
    bpl @commit
    jsr negate_result
@commit:
    jmp rt_commit_binary

rt_rem:
    jsr rt_need_two
    lda rt_rhs
    ora rt_rhs+1
    bne @nonzero
    jmp rt_rem_zero
@nonzero:
    jsr unsigned_divide
    lda work_r
    sta rt_result
    lda work_r+1
    sta rt_result+1
    lda work_sign
    and #$40
    beq @commit
    jsr negate_result
@commit:
    jmp rt_commit_binary

; Absolute values are unsigned bit patterns. Negating $8000 leaves $8000,
; which is exactly the unsigned magnitude needed for signed division.
unsigned_divide:
    lda rt_lhs+1
    and #$80
    lsr a
    sta work_sign               ; dividend sign in bit 6
    lda rt_lhs+1
    eor rt_rhs+1
    and #$80
    ora work_sign
    sta work_sign

    lda rt_lhs
    sta work_q
    lda rt_lhs+1
    sta work_q+1
    bpl @lhs_abs_done
    sec
    lda #$00
    sbc work_q
    sta work_q
    lda #$00
    sbc work_q+1
    sta work_q+1
@lhs_abs_done:
    lda rt_rhs
    sta work_d
    lda rt_rhs+1
    sta work_d+1
    bpl @rhs_abs_done
    sec
    lda #$00
    sbc work_d
    sta work_d
    lda #$00
    sbc work_d+1
    sta work_d+1
@rhs_abs_done:
    lda #$00
    sta work_r
    sta work_r+1
    sta work_r+2
    ldx #16
@bit:
    asl work_q
    rol work_q+1
    rol work_r
    rol work_r+1
    rol work_r+2
    lda work_r+2
    bne @subtract
    lda work_r+1
    cmp work_d+1
    bcc @next
    bne @subtract
    lda work_r
    cmp work_d
    bcc @next
@subtract:
    sec
    lda work_r
    sbc work_d
    sta work_r
    lda work_r+1
    sbc work_d+1
    sta work_r+1
    lda work_r+2
    sbc #$00
    sta work_r+2
    inc work_q                  ; shifted quotient's low bit is clear
@next:
    dex
    bne @bit
    rts

negate_result:
    sec
    lda #$00
    sbc rt_result
    sta rt_result
    lda #$00
    sbc rt_result+1
    sta rt_result+1
    rts

; On return, C means lhs >= rhs and Z means lhs = rhs, in signed order.
; Flipping the high-byte sign bit maps signed order to unsigned order.
compare_cells:
    jsr rt_need_two
    lda rt_lhs+1
    eor #$80
    sta compare_hi
    lda rt_rhs+1
    eor #$80
    sta compare_hi+1
    lda compare_hi
    cmp compare_hi+1
    bne @done
    lda rt_lhs
    cmp rt_rhs
@done:
    rts

rt_eq:
    jsr compare_cells
    beq result_true
    bne result_false

rt_ne:
    jsr compare_cells
    bne result_true
    beq result_false

rt_lt:
    jsr compare_cells
    bcc result_true
    bcs result_false

rt_le:
    jsr compare_cells
    bcc result_true
    beq result_true
    bne result_false

rt_gt:
    jsr compare_cells
    bcc result_false
    beq result_false
    bne result_true

rt_ge:
    jsr compare_cells
    bcs result_true
    bcc result_false

result_true:
    lda #$01
    sta rt_result
    lda #$00
    sta rt_result+1
    jmp rt_commit_binary

result_false:
    lda #$00
    sta rt_result
    lda #$00
    sta rt_result+1
    jmp rt_commit_binary
