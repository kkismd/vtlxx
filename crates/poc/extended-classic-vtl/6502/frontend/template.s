; Compile-run-local registry for saved P0a template source.
.setcpu "6502"

.export template_reset, template_is_bound, template_publish, template_lookup
.importzp cc_status, cc_arg, cc_data_length

.segment "ZEROPAGE"
template_slot: .res 1

.segment "BSS"
template_valid: .res 26
template_starts: .res 52
template_lengths: .res 52

.segment "CODE"
template_reset:
    lda #0
    ldx #25
@clear:
    sta template_valid,x
    dex
    bpl @clear
    rts

; A = lowercase ASCII identity. Read only; A=1 if published, else A=0.
template_is_bound:
    tax
    jsr template_select
    bcs @invalid
    lda template_valid,x
    rts
@invalid:
    lda #0
    rts

; A = identity, cc_arg = start address, cc_data_length = length.
; Validate all inputs before storing; publish valid last. cc_status is
; preserved on success and set to misuse (3) on invalid or duplicate input.
template_publish:
    pha
    tax
    jsr template_select
    bcs @bad_pop
    lda template_valid,x
    bne @bad_pop
    lda cc_data_length
    ora cc_data_length+1
    beq @bad_pop
    txa
    asl a
    tay
    lda cc_arg
    sta template_starts,y
    lda cc_arg+1
    sta template_starts+1,y
    lda cc_data_length
    sta template_lengths,y
    lda cc_data_length+1
    sta template_lengths+1,y
    ldx template_slot
    lda #1
    sta template_valid,x
    pla
    rts
@bad_pop:
    pla
    lda #3
    sta cc_status
    rts

; A = identity. Carry set if unavailable. On success cc_arg=start address
; and cc_data_length=length. Other status and table state are unchanged.
template_lookup:
    tax
    jsr template_select
    bcs @missing
    lda template_valid,x
    beq @missing
    txa
    asl a
    tay
    lda template_starts,y
    sta cc_arg
    lda template_starts+1,y
    sta cc_arg+1
    lda template_lengths,y
    sta cc_data_length
    lda template_lengths+1,y
    sta cc_data_length+1
    clc
    rts
@missing:
    sec
    rts

; X = ASCII a..z. Returns X=slot, carry clear; invalid sets carry.
template_select:
    txa
    sec
    sbc #'a'
    cmp #26
    bcs @invalid
    tax
    stx template_slot
    clc
    rts
@invalid:
    sec
    rts
