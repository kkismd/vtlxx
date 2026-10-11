; Read-only virtual source for one P0a template expansion.
; init: cc_arg=start, cc_data_length=u16 length, Y=replacement; clobbers A.
; next: A=byte/carry clear, or carry set for EOF/error; clobbers A,Y,flags.
; template_reader_error distinguishes malformed saved source from virtual EOF.
; stop clobbers A. The module uses only its private ZP cells and never calls
; fe_next, template_lookup, or serial input.
.setcpu "6502"

.export template_reader_init, template_reader_next, template_reader_stop
.exportzp template_reader_error
.importzp cc_arg, cc_data_length

.segment "ZEROPAGE"
template_reader_ptr:       .res 2
template_reader_remaining: .res 2
template_reader_replacement: .res 1
template_reader_error:     .res 1
template_reader_char:      .res 1

.segment "CODE"
; Start is cc_arg, length is cc_data_length, and Y is the replacement byte.
template_reader_init:
    lda cc_arg
    sta template_reader_ptr
    lda cc_arg+1
    sta template_reader_ptr+1
    lda cc_data_length
    sta template_reader_remaining
    lda cc_data_length+1
    sta template_reader_remaining+1
    sty template_reader_replacement
    lda #0
    sta template_reader_error
    rts

template_reader_stop:
    lda #0
    sta template_reader_remaining
    sta template_reader_remaining+1
    rts

; Return A=byte, carry clear; carry set means virtual EOF. A malformed stored
; brace pair sets template_reader_error and is reported as frontend syntax.
template_reader_next:
    lda template_reader_remaining
    ora template_reader_remaining+1
    beq @eof
    ldy #0
    lda (template_reader_ptr),y
    sta template_reader_char
    jsr @advance
    lda template_reader_char
    cmp #'{'
    bne @return_char
    lda template_reader_remaining
    ora template_reader_remaining+1
    beq @bad
    ldy #0
    lda (template_reader_ptr),y
    cmp #'}'
    bne @bad
    jsr @advance
    lda template_reader_replacement
    clc
    rts
@return_char:
    cmp #'}'
    beq @bad
    lda template_reader_char
    clc
    rts
@bad:
    lda #1
    sta template_reader_error
@eof:
    sec
    rts
@advance:
    inc template_reader_ptr
    bne :+
    inc template_reader_ptr+1
:
    lda template_reader_remaining
    bne :+
    dec template_reader_remaining+1
:
    dec template_reader_remaining
    rts
