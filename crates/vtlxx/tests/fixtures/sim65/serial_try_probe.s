.setcpu "6502"
.export _main, test_input_events, test_input_end
.import serial_try_in, serial_in, serial_out, halt

.segment "DATA"
; NotReady, A, NUL, FF, EOF, then a deliberately unreachable Fatal.
test_input_events:
    .byte 1,0, 0,65, 0,0, 0,255, 2,0, 3,0
test_input_end:

.segment "CODE"
_main:
    jsr serial_try_in
    cpx #1
    bne @fail1                   ; NotReady
    jsr serial_try_in
    cpx #0
    bne @fail2
    cmp #65
    bne @fail3
    jsr serial_in
    bcs @fail8
    cmp #0
    bne @fail9
    jsr serial_try_in
    cpx #0
    bne @fail4
    cmp #255
    bne @fail5
    jsr serial_try_in
    cpx #2
    bne @fail6
    jsr serial_try_in
    cpx #2
    bne @fail7
    lda #'O'
    jsr serial_out
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
@fail1:
    lda #9
    jmp halt
@fail2:
    lda #10
    jmp halt
@fail3:
    lda #11
    jmp halt
@fail4:
    lda #12
    jmp halt
@fail5:
    lda #13
    jmp halt
@fail6:
    lda #14
    jmp halt
@fail7:
    lda #15
    jmp halt
@fail8:
    lda #16
    jmp halt
@fail9:
    lda #17
    jmp halt
