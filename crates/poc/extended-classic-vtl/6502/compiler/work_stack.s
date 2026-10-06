; Source-phase u16 work stack, separate from runtime Cell value stack.
.setcpu "6502"

.export cc_work_reset, cc_work_push, cc_work_pop, cc_work_swap
.exportzp cc_work_depth
.importzp cc_status

.segment "ZEROPAGE"
cc_work_depth: .res 1
work_low:      .res 1
work_high:     .res 1

.segment "BSS"
work_words: .res 32

.segment "CODE"
cc_work_reset:
    lda #0
    sta cc_work_depth
    rts

; A/X = compiler-private u16 word.
cc_work_push:
    sta work_low
    stx work_high
    lda cc_status
    bne @done
    ldx cc_work_depth
    cpx #16
    bcc @space
    lda #9
    sta cc_status
    rts
@space:
    txa
    asl a
    tay
    lda work_low
    sta work_words,y
    lda work_high
    sta work_words+1,y
    inc cc_work_depth
@done:
    rts

; A/X = popped compiler-private u16 word.
cc_work_pop:
    lda cc_status
    bne @fail
    lda cc_work_depth
    bne @nonempty
    lda #9
    sta cc_status
    bne @fail
@nonempty:
    dec cc_work_depth
    lda cc_work_depth
    asl a
    tay
    lda work_words,y
    pha
    lda work_words+1,y
    tax
    pla
    rts
@fail:
    lda #0
    tax
    rts

cc_work_swap:
    lda cc_status
    bne @done
    lda cc_work_depth
    cmp #2
    bcs @enough
    lda #9
    sta cc_status
    rts
@enough:
    sec
    sbc #2
    asl a
    tay
    lda work_words,y
    sta work_low
    lda work_words+1,y
    sta work_high
    lda work_words+2,y
    sta work_words,y
    lda work_words+3,y
    sta work_words+1,y
    lda work_low
    sta work_words+2,y
    lda work_high
    sta work_words+3,y
@done:
    rts
