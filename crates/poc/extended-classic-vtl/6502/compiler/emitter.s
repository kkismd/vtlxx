; Fixed native templates. All multi-byte forms reserve their full size first.
.setcpu "6502"

.export cc_push_const, cc_call, cc_return, cc_jump, cc_jz
.export cc_load_reg, cc_store_reg
.export cc_jump_placeholder, cc_jz_placeholder, cc_patch_here, cc_jump_to
.import cc_reserve, cc_write_byte_raw, cc_mark, cc_patch, cc_track_patch
.import rt_push, rt_pop_condition, rt_load_reg, rt_store_reg
.importzp cc_status, cc_arg, cc_cursor

.segment "ZEROPAGE"
em_word:  .res 2
em_patch: .res 2
em_index: .res 1

.segment "CODE"
; A/X = raw Cell bits. LDA #low; LDX #high; JSR rt_push.
cc_push_const:
    sta em_word
    stx em_word+1
    lda #7
    jsr cc_reserve
    lda cc_status
    bne @done
    lda #$a9
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda #$a2
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
    lda #$20
    jsr cc_write_byte_raw
    lda #<rt_push
    jsr cc_write_byte_raw
    lda #>rt_push
    jsr cc_write_byte_raw
@done:
    rts

; A/X = already resolved executable/helper address.
cc_call:
    sta em_word
    stx em_word+1
    lda #3
    jsr cc_reserve
    lda cc_status
    bne @done
    lda #$20
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

; X = fixed register index 0..25. The backend owns the LDX immediate and
; helper-call encoding; source classification stays in the frontend.
cc_load_reg:
    lda #<rt_load_reg
    ldy #>rt_load_reg
    jmp cc_indexed_call

cc_store_reg:
    lda #<rt_store_reg
    ldy #>rt_store_reg
    jmp cc_indexed_call

cc_indexed_call:
    stx em_index
    sta em_word
    sty em_word+1
    lda #5
    jsr cc_reserve
    lda cc_status
    bne @done
    lda #$a2
    jsr cc_write_byte_raw
    lda em_index
    jsr cc_write_byte_raw
    lda #$20
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

cc_return:
    lda #1
    jsr cc_reserve
    lda cc_status
    bne @done
    lda #$60
    jsr cc_write_byte_raw
@done:
    rts

; A/X = absolute target. JMP abs16.
cc_jump:
cc_jump_to:
    sta em_word
    stx em_word+1
    lda #3
    jsr cc_reserve
    lda cc_status
    bne @done
    lda #$4c
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

; A/X = absolute target. Pop one condition; nonzero skips absolute JMP.
cc_jz:
    sta em_word
    stx em_word+1
    lda #8
    jsr cc_reserve
    lda cc_status
    bne @done
    lda #$20
    jsr cc_write_byte_raw
    lda #<rt_pop_condition
    jsr cc_write_byte_raw
    lda #>rt_pop_condition
    jsr cc_write_byte_raw
    lda #$d0
    jsr cc_write_byte_raw
    lda #3
    jsr cc_write_byte_raw
    lda #$4c
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

; Patch address is the low byte of the absolute operand.
cc_jump_placeholder:
    lda cc_cursor
    clc
    adc #1
    sta em_patch
    lda cc_cursor+1
    adc #0
    sta em_patch+1
    lda #0
    tax
    jsr cc_jump
    lda cc_status
    bne @failed
    lda em_patch
    ldx em_patch+1
    jsr cc_track_patch
@failed:
    lda em_patch
    ldx em_patch+1
    rts

cc_jz_placeholder:
    lda cc_cursor
    clc
    adc #6
    sta em_patch
    lda cc_cursor+1
    adc #0
    sta em_patch+1
    lda #0
    tax
    jsr cc_jz
    lda cc_status
    bne @failed
    lda em_patch
    ldx em_patch+1
    jsr cc_track_patch
@failed:
    lda em_patch
    ldx em_patch+1
    rts

cc_patch_here:
    sta em_patch
    stx em_patch+1
    jsr cc_mark
    sta cc_arg
    stx cc_arg+1
    lda em_patch
    ldx em_patch+1
    jmp cc_patch
