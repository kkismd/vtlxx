; Direct Write / SourceProcedure bindings for lowercase source identities.
.setcpu "6502"

.export cc_binding_reset, cc_publish, cc_resolve
.import cc_is_completed
.importzp cc_status, cc_arg

.segment "ZEROPAGE"
binding_slot: .res 1

.segment "BSS"
binding_valid: .res 52
binding_target: .res 104

.segment "CODE"
cc_binding_reset:
    lda #0
    ldx #51
@clear:
    sta binding_valid,x
    dex
    bpl @clear
    rts

; A = role (0 Write, 1 SourceProcedure), X = ASCII identity a..z.
; cc_arg = completed executable target. A target is never published early.
cc_publish:
    jsr binding_select
    lda cc_status
    bne @done
    ldx binding_slot
    lda binding_valid,x
    beq @available
    lda #8
    sta cc_status
    rts
@available:
    lda cc_arg
    ldx cc_arg+1
    jsr cc_is_completed
    lda cc_status
    bne @done
    lda binding_slot
    asl a
    tax
    lda cc_arg
    sta binding_target,x
    lda cc_arg+1
    sta binding_target+1,x
    ldx binding_slot
    lda #1
    sta binding_valid,x
@done:
    rts

; A/X = already resolved target. No source character reaches runtime code.
cc_resolve:
    jsr binding_select
    lda cc_status
    bne @fail
    ldx binding_slot
    lda binding_valid,x
    bne @bound
    lda #8
    sta cc_status
    bne @fail
@bound:
    txa
    asl a
    tax
    lda binding_target,x
    pha
    lda binding_target+1,x
    tax
    pla
    rts
@fail:
    lda #0
    tax
    rts

binding_select:
    ldy cc_status
    bne @done
    cmp #2
    bcs @bad
    sta binding_slot
    txa
    sec
    sbc #'a'
    cmp #26
    bcs @bad
    ldx binding_slot
    beq @write
    clc
    adc #26
@write:
    sta binding_slot
@done:
    rts
@bad:
    lda #8
    sta cc_status
    rts
