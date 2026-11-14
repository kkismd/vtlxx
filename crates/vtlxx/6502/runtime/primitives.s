; Runtime I/O helpers use only the #273 serial adapters.
.setcpu "6502"

.export rt_print_char, rt_print_number, rt_print_bytes, rt_read_char, rt_read_entropy
.import rt_need_one, rt_overflow, rt_push, serial_in, serial_out, entropy16, halt
.importzp rt_depth, rt_stack, rt_input_eof

.importzp zp_phase
number_low = zp_phase + 93
number_high = zp_phase + 94
number_digit = zp_phase + 95
number_place = zp_phase + 96
number_started = zp_phase + 97
bytes_pointer = zp_phase + 98

.segment "BSS"
bytes_remaining: .res 1

.segment "RODATA"
decimal_low:  .byte <10000, <1000, <100, <10, <1
decimal_high: .byte >10000, >1000, >100, >10, >1

.segment "CODE"
; Blocking PrimaryRead: discard physical NUL, map transport EOF to -1, and
; commit exactly one Cell only after the input result is known.
rt_read_char:
    lda rt_depth
    cmp #16
    bcc @capacity_ready
    jmp rt_overflow
@capacity_ready:
    lda rt_input_eof
    bne @eof
@read:
    jsr serial_in
    bcs @remember_eof
    cmp #0
    beq @read
    ldx #0
    jmp rt_push

@remember_eof:
    lda #1
    sta rt_input_eof
@eof:
    lda #$ff
    ldx #$ff
    jmp rt_push

; Read one target-provided raw16 Cell. Check capacity before calling the
; provider, then commit only after it reports success (A=low, X=high, C=0).
rt_read_entropy:
    lda rt_depth
    cmp #16
    bcc @capacity_ready
    jmp rt_overflow
@capacity_ready:
    jsr entropy16
    bcs @unavailable
    jmp rt_push
@unavailable:
    lda #5
    jmp halt

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

; Target-private ABI: A/X = address, Y = byte count (1..255). The adapter
; may clobber all registers and flags, so keep the cursor and count in RAM.
rt_print_bytes:
    sta bytes_pointer
    stx bytes_pointer+1
    sty bytes_remaining
@next_byte:
    ldy #0
    lda (bytes_pointer),y
    jsr serial_out
    inc bytes_pointer
    bne @pointer_ready
    inc bytes_pointer+1
@pointer_ready:
    dec bytes_remaining
    bne @next_byte
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
