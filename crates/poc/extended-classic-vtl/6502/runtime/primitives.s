; Output helpers use only the #273 SERIAL_OUT adapter.
.setcpu "6502"

.export rt_print_char, rt_print_number
.import rt_need_one, serial_out
.importzp rt_depth, rt_stack

.segment "ZEROPAGE"
number_low:     .res 1
number_high:    .res 1
number_digit:   .res 1
number_place:   .res 1
number_started: .res 1

.segment "RODATA"
decimal_low:  .byte <10000, <1000, <100, <10, <1
decimal_high: .byte >10000, >1000, >100, >10, >1

.segment "CODE"
rt_print_char:
    jsr rt_need_one
    lda rt_depth
    sec
    sbc #1
    asl a
    tax
    lda rt_stack,x
    jsr serial_out
    dec rt_depth
    rts

rt_print_number:
    jsr rt_need_one
    lda rt_depth
    sec
    sbc #1
    asl a
    tax
    lda rt_stack,x
    sta number_low
    lda rt_stack+1,x
    sta number_high
    bpl @magnitude_ready
    lda #'-'
    jsr serial_out
    ; Unsigned magnitude also handles -32768 (0x8000).
    sec
    lda #0
    sbc number_low
    sta number_low
    lda #0
    sbc number_high
    sta number_high
@magnitude_ready:
    lda #0
    sta number_started
    sta number_place
@place:
    lda #0
    sta number_digit
@subtract:
    ldx number_place
    lda number_high
    cmp decimal_high,x
    bcc @digit_ready
    bne @subtract_value
    lda number_low
    cmp decimal_low,x
    bcc @digit_ready
@subtract_value:
    sec
    lda number_low
    sbc decimal_low,x
    sta number_low
    lda number_high
    sbc decimal_high,x
    sta number_high
    inc number_digit
    jmp @subtract
@digit_ready:
    lda number_digit
    bne @emit
    lda number_started
    bne @emit
    lda number_place
    cmp #4
    bne @next
@emit:
    lda #1
    sta number_started
    lda number_digit
    clc
    adc #'0'
    jsr serial_out
@next:
    inc number_place
    lda number_place
    cmp #5
    bne @place
    dec rt_depth
    rts
