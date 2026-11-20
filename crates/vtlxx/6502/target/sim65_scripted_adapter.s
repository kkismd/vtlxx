; Test-only scripted input bridge. Link instead of sim65_adapter.s.
.setcpu "6502"

.export serial_in, serial_try_in, serial_out, halt, entropy16
.exportzp script_cursor
.export script_read_calls
.export script_try_calls
.import _putchar, exit
.import test_input_events, test_input_end

.segment "ZEROPAGE"
script_cursor: .res 2
script_range_low: .res 1

.segment "DATA"
script_eof: .byte 0
script_initialized: .byte 0
script_read_calls: .byte 0
script_try_calls: .byte 0

.segment "CODE"
serial_try_in:
    inc script_try_calls
    lda script_initialized
    bne @initialized
    lda #<test_input_events
    sta script_cursor
    lda #>test_input_events
    sta script_cursor+1
    lda #1
    sta script_initialized
@initialized:
    lda script_eof
    bne @eof
    jsr script_next
    cmp #0
    beq @byte
    cmp #1
    beq @not_ready
    cmp #2
    beq @set_eof
    ldx #3
    rts
@byte:
    txa
    ldx #0
    rts
@not_ready:
    ldx #1
    rts
@set_eof:
    lda #1
    sta script_eof
@eof:
    ldx #2
    rts

; Wait input ignores NotReady events but preserves their event ordering.
serial_in:
    inc script_read_calls
@again:
    jsr serial_try_in
    cpx #1
    beq @again
    cpx #0
    beq @success
    cpx #2
    beq @ended
    lda #6
    jmp halt
@success:
    clc
    rts
@ended:
    sec
    rts

; @next returns event tag in A, and event payload in X for tag 0.
; Events are pairs: tag, byte. Tag 0=Byte, 1=NotReady, 2=EOF, 3=Fatal.
script_next:
    ; Require cursor >= start before calculating the number of bytes left.
    lda script_cursor+1
    cmp #>test_input_events
    bcc @fixture_bad
    bne @remaining
    lda script_cursor
    cmp #<test_input_events
    bcc @fixture_bad
@remaining:
    ; Compute end - cursor. A complete pair requires at least two bytes.
    lda #<test_input_end
    sec
    sbc script_cursor
    sta script_range_low
    lda #>test_input_end
    sbc script_cursor+1
    bcc @fixture_bad
    bne @read_event
    lda script_range_low
    cmp #2
    bcc @fixture_bad
@read_event:
    ldy #0
    lda (script_cursor),y
    pha
    cmp #4
    bcs @bad_tag
    iny
    lda (script_cursor),y
    tax
    clc
    lda script_cursor
    adc #2
    sta script_cursor
    bcc @no_carry
    inc script_cursor+1
@no_carry:
    pla
    rts
@bad_tag:
    pla
@fixture_bad:
    lda #$40                     ; Malformed fixture, distinct from I/O Fatal.
    jmp halt


serial_out:
    ldx #0
    jmp _putchar

halt:
    jmp exit

entropy16:
    sec
    rts
