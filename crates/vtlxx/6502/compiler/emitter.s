; Fixed native templates. All multi-byte forms reserve their full size first.
.setcpu "6502"
.import session_backend_error

.export cc_push_const, cc_call, cc_return, cc_jump, cc_jz
.export cc_load_reg, cc_store_reg
.export cc_jump_placeholder, cc_jz_placeholder, cc_patch_here, cc_jump_to
.export cc_literal_output_begin, cc_literal_output_byte, cc_literal_output_finish
.export cc_static_reset, cc_static_begin, cc_static_byte, cc_static_finish, cc_static_read
.export cc_static_start, cc_static_length
.import cc_reserve, cc_write_byte_raw, cc_emit_byte, cc_mark, cc_patch, cc_track_patch
.import rt_push, rt_pop_condition, rt_load_reg, rt_store_reg, rt_print_bytes
.import rt_read_static
.importzp rt_lhs, rt_rhs
.import owner_require_active
.importzp cc_status, cc_arg, cc_cursor

.importzp zp_phase
em_word = zp_phase + 62
em_patch = zp_phase + 64
em_index = zp_phase + 66

.segment "BSS"
literal_patch: .res 2
literal_start: .res 2
literal_count: .res 1
literal_byte:  .res 1
cc_static_start: .res 2
cc_static_length: .res 2
static_patch: .res 2
static_active: .res 1
static_byte: .res 1

.segment "CODE"
; Static payload operations are sequential and require an active top-level
; owner. A fatal error aborts the compile session; only successful
; finish exposes cc_static_start/cc_static_length to the frontend.
cc_static_reset:
    lda #0
    sta static_active
    rts

cc_static_begin:
    jsr owner_require_active
    lda static_active
    bne @bad
    jsr cc_jump_placeholder
    sta static_patch
    stx static_patch+1
    jsr cc_mark
    sta cc_static_start
    stx cc_static_start+1
    lda #0
    sta cc_static_length
    sta cc_static_length+1
    lda #1
    sta static_active
@done:
    rts
@bad:
    lda #3
    jmp session_backend_error

cc_static_byte:
    sta static_byte
    lda static_active
    beq @bad
    ; Length 32768 is the largest useful payload.
    lda cc_static_length+1
    cmp #$80
    bcs @bad
    lda static_byte
    jsr cc_emit_byte
    inc cc_static_length
    bne @done
    inc cc_static_length+1
@done:
    rts
@bad:
    lda #3
    jmp session_backend_error

cc_static_finish:
    lda static_active
    beq @bad
    lda static_patch
    ldx static_patch+1
    jsr cc_patch_here
    lda #0
    sta static_active
@done:
    rts
@bad:
    lda #3
    jmp session_backend_error

; Caller sets cc_static_start and cc_static_length to a finished payload.
; Emit four immediate stores followed by the bounds-checked helper call.
cc_static_read:
    lda #19
    jsr cc_reserve
    ldx #0
@loop:
    lda #$a9
    jsr cc_write_byte_raw
    lda cc_static_start,x
    jsr cc_write_byte_raw
    lda #$85
    jsr cc_write_byte_raw
    txa
    clc
    adc #<rt_lhs
    jsr cc_write_byte_raw
    inx
    cpx #4
    bne @loop
    lda #$20
    jsr cc_write_byte_raw
    lda #<rt_read_static
    jsr cc_write_byte_raw
    lda #>rt_read_static
    jsr cc_write_byte_raw
@done:
    rts

; Stream bytes into code arena chunks, with a tracked jump over each payload.
; An empty literal emits nothing. State is private to this sequential emitter.
cc_literal_output_begin:
    lda #0
    sta literal_count
    rts

; A = next validated content byte. A full chunk is finished only when the
; following byte arrives, so each emitted chunk has a count in 1..255.
cc_literal_output_byte:
    sta literal_byte
    lda literal_count
    cmp #255
    bne @start_if_needed
    jsr cc_literal_output_finish
@start_if_needed:
    lda literal_count
    bne @append
    jsr cc_jump_placeholder
    sta literal_patch
    stx literal_patch+1
    jsr cc_mark
    sta literal_start
    stx literal_start+1
@append:
    lda literal_byte
    jsr cc_emit_byte
    inc literal_count
@done:
    rts

; Reserve the entire fixed call template before patching or writing it.
cc_literal_output_finish:
    lda literal_count
    beq @done
    lda #9
    jsr cc_reserve
    lda literal_patch
    ldx literal_patch+1
    jsr cc_patch_here
    lda #$a9               ; LDA #<literal_start
    jsr cc_write_byte_raw
    lda literal_start
    jsr cc_write_byte_raw
    lda #$a2               ; LDX #>literal_start
    jsr cc_write_byte_raw
    lda literal_start+1
    jsr cc_write_byte_raw
    lda #$a0               ; LDY #count
    jsr cc_write_byte_raw
    lda literal_count
    jsr cc_write_byte_raw
    lda #$20               ; JSR rt_print_bytes
    jsr cc_write_byte_raw
    lda #<rt_print_bytes
    jsr cc_write_byte_raw
    lda #>rt_print_bytes
    jsr cc_write_byte_raw
    lda #0
    sta literal_count
@done:
    rts

; A/X = raw Cell bits. LDA #low; LDX #high; JSR rt_push.
cc_push_const:
    sta em_word
    stx em_word+1
    lda #7
    jsr cc_reserve
    lda #$a9
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda #$a2
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
    lda #$20
    jsr cc_write_byte_raw
    lda #<rt_push
    jsr cc_write_byte_raw
    lda #>rt_push
    jsr cc_write_byte_raw
@done:
    rts

; A/X = already resolved executable/helper address.
cc_call:
    sta em_word
    stx em_word+1
    lda #3
    jsr cc_reserve
    lda #$20
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

; X = fixed register index 0..25. The backend owns the LDX immediate and
; helper-call encoding; source classification stays in the frontend.
cc_load_reg:
    lda #<rt_load_reg
    ldy #>rt_load_reg
    jmp cc_indexed_call

cc_store_reg:
    lda #<rt_store_reg
    ldy #>rt_store_reg
    jmp cc_indexed_call

cc_indexed_call:
    stx em_index
    sta em_word
    sty em_word+1
    lda #5
    jsr cc_reserve
    lda #$a2
    jsr cc_write_byte_raw
    lda em_index
    jsr cc_write_byte_raw
    lda #$20
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

cc_return:
    lda #1
    jsr cc_reserve
    lda #$60
    jsr cc_write_byte_raw
@done:
    rts

; A/X = absolute target. JMP abs16.
cc_jump:
cc_jump_to:
    sta em_word
    stx em_word+1
    lda #3
    jsr cc_reserve
    lda #$4c
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

; A/X = absolute target. Pop one condition; nonzero skips absolute JMP.
cc_jz:
    sta em_word
    stx em_word+1
    lda #8
    jsr cc_reserve
    lda #$20
    jsr cc_write_byte_raw
    lda #<rt_pop_condition
    jsr cc_write_byte_raw
    lda #>rt_pop_condition
    jsr cc_write_byte_raw
    lda #$d0
    jsr cc_write_byte_raw
    lda #3
    jsr cc_write_byte_raw
    lda #$4c
    jsr cc_write_byte_raw
    lda em_word
    jsr cc_write_byte_raw
    lda em_word+1
    jsr cc_write_byte_raw
@done:
    rts

; Patch address is the low byte of the absolute operand.
cc_jump_placeholder:
    lda cc_cursor
    clc
    adc #1
    sta em_patch
    lda cc_cursor+1
    adc #0
    sta em_patch+1
    lda #0
    tax
    jsr cc_jump
    lda em_patch
    ldx em_patch+1
    jsr cc_track_patch
    lda em_patch
    ldx em_patch+1
    rts

cc_jz_placeholder:
    lda cc_cursor
    clc
    adc #6
    sta em_patch
    lda cc_cursor+1
    adc #0
    sta em_patch+1
    lda #0
    tax
    jsr cc_jz
    lda em_patch
    ldx em_patch+1
    jsr cc_track_patch
    lda em_patch
    ldx em_patch+1
    rts

cc_patch_here:
    sta em_patch
    stx em_patch+1
    jsr cc_mark
    sta cc_arg
    stx cc_arg+1
    lda em_patch
    ldx em_patch+1
    jmp cc_patch
