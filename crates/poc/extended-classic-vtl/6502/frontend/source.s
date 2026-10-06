; Bounded, one-pass source framing reader for an ECVTL compile-run.
; Input: u16 little-endian byte length, exactly that many source bytes,
; then the caller's runtime input. This module owns no source buffer.
.setcpu "6502"

.export fe_init, fe_next
.exportzp fe_remaining, fe_status
.import serial_in

.segment "ZEROPAGE"
fe_remaining: .res 2
fe_status:    .res 1

.segment "CODE"
; Read the frame length. Carry is clear on success, set on truncated header.
fe_init:
    jsr serial_in
    cmp #$ff
    beq @truncated
    sta fe_remaining
    jsr serial_in
    cmp #$ff
    beq @truncated
    sta fe_remaining+1
    lda #0
    sta fe_status
    clc
    rts
@truncated:
    lda #1
    sta fe_status
    sec
    rts

; Return A=next source byte with carry clear; carry set means exact frame end.
; A source read returning $ff is premature EOF and latches failure status 1.
; The caller must not call serial_in after carry is set.
fe_next:
    lda fe_status
    bne @failed
    lda fe_remaining
    ora fe_remaining+1
    beq @end
    jsr serial_in
    cmp #$ff
    beq @truncated
    pha
    lda fe_remaining
    bne @dec_low
    dec fe_remaining+1
@dec_low:
    dec fe_remaining
    pla
    clc
    rts
@end:
    sec
    rts
@truncated:
    lda #1
    sta fe_status
@failed:
    sec
    rts
