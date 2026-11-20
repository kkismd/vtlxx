.setcpu "6502"
.export _main, test_input_events, test_input_end
.import serial_try_in, serial_in, serial_out, halt

.segment "DATA"
; Try NotReady, Try A, Wait NotReady + NUL, Try FF, Wait B, then EOF.
test_input_events:
    .byte 1,0, 0,65, 1,0, 0,0, 0,255, 0,66, 2,0, 3,0
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
    bne @fail9                  ; Wait skipped NotReady and consumed NUL.
    jsr serial_try_in
    cpx #0
    bne @fail12
    cmp #255
    bne @fail13
    jsr serial_in               ; Wait follows Try on the same event cursor.
    bcs @fail14
    cmp #66
    bne @fail15
    jsr serial_try_in
    cpx #2
    bne @fail16
    jsr serial_in               ; EOF is sticky across both entry points.
    bcc @fail17
    jsr serial_try_in
    cpx #2
    bne @fail18
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
@fail10:
    lda #18
    jmp halt
@fail12:
    lda #20
    jmp halt
@fail13:
    lda #21
    jmp halt
@fail14:
    lda #22
    jmp halt
@fail15:
    lda #23
    jmp halt
@fail16:
    lda #24
    jmp halt
@fail17:
    lda #25
    jmp halt
@fail18:
    lda #26
    jmp halt
