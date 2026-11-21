.export _capcheck
.import _cap_a, _cap_x, _cap_y
.segment "CODE"
_capcheck:
    ldx #$ff
    jsr $fff0
    sta _cap_a
    stx _cap_x
    sty _cap_y
    rts
