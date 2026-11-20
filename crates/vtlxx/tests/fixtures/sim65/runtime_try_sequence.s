.setcpu "6502"
.export _main, test_input_events, test_input_end
.import rt_init, rt_try_read_char
.import script_try_calls, script_cursor
.import serial_out, halt
.importzp rt_depth, rt_stack, rt_input_eof

.segment "RODATA"
test_input_events:
    .byte 1, 0          ; NotReady -> 0
    .byte 0, 0          ; physical NUL -> 0
    .byte 0, 127        ; byte 127
    .byte 0, 255        ; byte 255 is not EOF
    .byte 2, 0          ; EOF -> -1
test_input_end:

.segment "CODE"
_main:
    jsr rt_init
    jsr rt_try_read_char
    jsr rt_try_read_char
    jsr rt_try_read_char
    jsr rt_try_read_char
    jsr rt_try_read_char
    jsr rt_try_read_char      ; sticky EOF must not call provider again
    lda rt_depth
    cmp #6
    bne @bad
    lda rt_stack
    ora rt_stack+1
    ora rt_stack+2
    ora rt_stack+3
    ora rt_stack+5
    ora rt_stack+7
    bne @bad
    lda rt_stack+4
    cmp #127
    bne @bad
    lda rt_stack+6
    cmp #255
    bne @bad
    lda rt_stack+8
    cmp #$ff
    bne @bad
    lda rt_stack+9
    cmp #$ff
    bne @bad
    lda rt_stack+10
    cmp #$ff
    bne @bad
    lda rt_stack+11
    cmp #$ff
    bne @bad
    lda rt_input_eof
    cmp #1
    bne @bad
    lda script_try_calls
    cmp #5
    bne @bad
    lda script_cursor
    cmp #<test_input_end
    bne @bad
    lda script_cursor+1
    cmp #>test_input_end
    bne @bad
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
@bad:
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt
