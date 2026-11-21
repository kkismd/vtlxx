.export _probe
.import _status, _value
.segment "CODE"
_probe:
    jsr $fff0
    stx _status
    sta _value
    rts
