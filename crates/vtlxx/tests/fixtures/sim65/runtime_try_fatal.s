.setcpu "6502"
.export _main, rt_test_probe, test_input_events, test_input_end
.import rt_init, rt_push, rt_try_read_char
.import script_try_calls, serial_out, halt
.importzp rt_depth, rt_stack, rt_input_eof

.segment "RODATA"
test_input_events:
    .byte 3, 0
test_input_end:

.segment "CODE"
_main:
    jsr rt_init
    lda #$34
    ldx #$12
    jsr rt_push
    jsr rt_try_read_char
    lda #99
    jmp halt

rt_test_probe:
    lda rt_depth
    cmp #1
    bne @bad
    lda rt_stack
    cmp #$34
    bne @bad
    lda rt_stack+1
    cmp #$12
    bne @bad
    lda rt_input_eof
    bne @bad
    lda script_try_calls
    cmp #1
    bne @bad
    lda #'K'
    jsr serial_out
    rts
@bad:
    lda #'F'
    jsr serial_out
    rts
