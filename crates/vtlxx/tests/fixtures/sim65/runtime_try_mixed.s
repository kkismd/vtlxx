.setcpu "6502"
.export _main, test_input_events, test_input_end
.import rt_init, rt_read_char, rt_try_read_char
.import script_try_calls, script_read_calls, script_cursor
.import serial_out, halt
.importzp rt_depth, rt_stack, rt_input_eof

.segment "RODATA"
test_input_events:
    .byte 1, 0          ; Try: NotReady
    .byte 0, 0          ; Wait: discard NUL
    .byte 0, 65         ; Wait: return A
    .byte 0, 66         ; Try: return B
    .byte 2, 0          ; Wait: discover EOF
test_input_end:

.segment "CODE"
_main:
    jsr rt_init
    jsr rt_try_read_char
    jsr rt_read_char
    jsr rt_try_read_char
    jsr rt_read_char
    jsr rt_try_read_char
    jsr rt_read_char
    lda rt_depth
    cmp #6
    bne @bad
    lda rt_stack
    ora rt_stack+1
    ora rt_stack+3
    ora rt_stack+5
    bne @bad
    lda rt_stack+2
    cmp #65
    bne @bad
    lda rt_stack+4
    cmp #66
    bne @bad
    ldx #6
@eof_values:
    lda rt_stack,x
    cmp #$ff
    bne @bad
    inx
    cpx #12
    bne @eof_values
    lda rt_input_eof
    cmp #1
    bne @bad
    lda script_try_calls
    cmp #5
    bne @bad
    lda script_read_calls
    cmp #3
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
