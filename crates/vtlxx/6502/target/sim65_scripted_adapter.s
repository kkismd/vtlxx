; Test-only scripted input bridge. Link instead of sim65_adapter.s.
.setcpu "6502"

.export serial_in, serial_try_in, serial_out, halt, entropy16
.import _putchar, exit
.import test_input_events, test_input_end

.segment "ZEROPAGE"
script_cursor: .res 2

.segment "DATA"
script_eof: .byte 0
script_initialized: .byte 0

.segment "CODE"
serial_try_in:
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
    lda script_cursor+1
    cmp #>test_input_end
    bne @read_event
    lda script_cursor
    cmp #<test_input_end
    bne @read_event
    lda #3                       ; Exhausted fixture is failure, never EOF.
    ldx #0
    rts
@read_event:
    ldy #0
    lda (script_cursor),y
    pha
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


serial_out:
    ldx #0
    jmp _putchar

halt:
    jmp exit

entropy16:
    sec
    rts
