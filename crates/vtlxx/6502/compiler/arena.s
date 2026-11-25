; Compiler-private, append-only 24 KiB code arena.
.setcpu "6502"
.import session_backend_error

.export cc_arena_reset, cc_mark, cc_emit_byte, cc_emit_u16, cc_patch
.export cc_patch_reset, cc_track_patch, cc_freeze
.export cc_reserve, cc_write_byte_raw
.export cc_arena, cc_patch_table
.exportzp cc_status, cc_arg, cc_cursor, cc_pending_count

.importzp zp_phase
.segment "ZEROPAGE"
cc_status: .res 1
cc_arg = zp_phase + 43
cc_cursor = zp_phase + 45
cc_frozen_end = zp_phase + 47
cc_ptr = zp_phase + 49
cc_limit = zp_phase + 51
cc_value = zp_phase + 53
cc_pending_count = zp_phase + 55
cc_reserve_count = zp_phase + 57

.segment "BSS"
cc_arena:     .res 24576
cc_arena_end:
cc_patch_table: .res 98

.segment "CODE"
cc_arena_reset:
    lda #<cc_arena
    sta cc_cursor
    sta cc_frozen_end
    lda #>cc_arena
    sta cc_cursor+1
    sta cc_frozen_end+1
    rts

; Bytes before this cursor belong to completed owners and cannot be patched.
cc_freeze:
    lda cc_cursor
    sta cc_frozen_end
    lda cc_cursor+1
    sta cc_frozen_end+1
    rts

cc_patch_reset:
    lda #0
    sta cc_pending_count
    sta cc_pending_count+1
    rts

; A/X = absolute address of next emitted byte.
cc_mark:
    lda cc_cursor
    ldx cc_cursor+1
    rts

; A = required code byte count (1..255). No cursor mutation on failure.
cc_reserve:
    sta cc_reserve_count
    clc
    lda cc_cursor
    adc cc_reserve_count
    sta cc_limit
    lda cc_cursor+1
    adc #0
    bcs @full
    sta cc_limit+1
    cmp #>cc_arena_end
    bcc @ok
    bne @full
    lda cc_limit
    cmp #<cc_arena_end
    bcc @ok
    beq @ok
@full:
    lda #1
    jmp session_backend_error
@ok:
@done:
    rts

; Caller must reserve the whole instruction first. A = byte to append.
cc_write_byte_raw:
    ldy #0
    sta (cc_cursor),y
    inc cc_cursor
    bne @done
    inc cc_cursor+1
@done:
    rts

cc_emit_byte:
    sta cc_value
    lda #1
    jsr cc_reserve
    lda cc_value
    jsr cc_write_byte_raw
@done:
    rts

; A/X = little-endian u16.
cc_emit_u16:
    sta cc_value
    stx cc_value+1
    lda #2
    jsr cc_reserve
    lda cc_value
    jsr cc_write_byte_raw
    lda cc_value+1
    jsr cc_write_byte_raw
@done:
    rts

; A/X = low-byte operand address; cc_arg = replacement u16.
; Both operand bytes must already have been emitted in this arena.
cc_patch:
    sta cc_ptr
    stx cc_ptr+1
    lda cc_ptr+1
    cmp #>cc_arena
    bcc @bad
    bne @upper
    lda cc_ptr
    cmp #<cc_arena
    bcc @bad
@upper:
    lda cc_ptr+1
    cmp cc_frozen_end+1
    bcc @bad
    bne @within_open_owner
    lda cc_ptr
    cmp cc_frozen_end
    bcc @bad
@within_open_owner:
    clc
    lda cc_ptr
    adc #2
    sta cc_limit
    lda cc_ptr+1
    adc #0
    bcs @bad
    cmp cc_cursor+1
    bcc @write
    bne @bad
    lda cc_limit
    cmp cc_cursor
    bcc @write
    bne @bad
@write:
    ; Each entry is a unique operand emitted by the placeholder routines.
    ldx #0
@scan:
    cpx cc_pending_count
    beq @bad
    txa
    asl a
    tay
    lda cc_patch_table,y
    cmp cc_ptr
    bne @next
    lda cc_patch_table+1,y
    cmp cc_ptr+1
    beq @found
@next:
    inx
    bne @scan
@found:
    ; Preserve the table offset in X while writing through cc_ptr.
    tya
    tax
    ldy #0
    lda cc_arg
    sta (cc_ptr),y
    iny
    lda cc_arg+1
    sta (cc_ptr),y
    dec cc_pending_count
    lda cc_pending_count
    asl a
    tay
    lda cc_patch_table,y
    sta cc_patch_table,x
    lda cc_patch_table+1,y
    sta cc_patch_table+1,x
    rts
@bad:
    lda #2
    jmp session_backend_error

; Internal emitter contract: A/X is the operand of a newly appended,
; fully reserved placeholder. Append order guarantees uniqueness.
cc_track_patch:
    sta cc_ptr
    stx cc_ptr+1
    lda cc_pending_count
    cmp #49
    bcs @full
    asl a
    tay
    lda cc_ptr
    sta cc_patch_table,y
    lda cc_ptr+1
    sta cc_patch_table+1,y
    inc cc_pending_count
    rts
@full:
    lda #2
    jmp session_backend_error
