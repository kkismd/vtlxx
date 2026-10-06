; EC07 compiler-private, append-only 2 KiB native code arena.
.setcpu "6502"

.export cc_arena_reset, cc_mark, cc_emit_byte, cc_emit_u16, cc_patch
.export cc_patch_reset, cc_track_patch, cc_freeze
.export cc_reserve, cc_write_byte_raw
.export cc_arena
.exportzp cc_status, cc_arg, cc_cursor, cc_pending_count

.segment "ZEROPAGE"
cc_status: .res 1
cc_arg:    .res 2
cc_cursor: .res 2
cc_frozen_end: .res 2
cc_ptr:    .res 2
cc_limit:  .res 2
cc_value:  .res 2
cc_pending_count: .res 2
cc_bit_index: .res 2
cc_bit_mask: .res 1

.segment "RODATA"
patch_masks: .byte 1,2,4,8,16,32,64,128

.segment "BSS"
cc_arena:     .res 2048
cc_arena_end:
patch_bits:   .res 256

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
    tax
@clear:
    sta patch_bits,x
    inx
    bne @clear
    rts

; A/X = absolute address of next emitted byte.
cc_mark:
    lda cc_cursor
    ldx cc_cursor+1
    rts

; A = required byte count (1..255). No cursor mutation on failure.
cc_reserve:
    pha
    lda cc_status
    beq @active
    pla
    rts
@active:
    pla
    clc
    adc cc_cursor
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
    sta cc_status
@ok:
    rts

; Caller must reserve the whole template first. A = byte to append.
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
    lda cc_status
    bne @done
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
    lda cc_status
    bne @done
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
    lda cc_status
    bne @done
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
    ldy #0
    lda cc_arg
    sta (cc_ptr),y
    iny
    lda cc_arg+1
    sta (cc_ptr),y
    lda cc_ptr
    ldx cc_ptr+1
    jsr cc_untrack_patch
    rts
@bad:
    lda #2
    sta cc_status
@done:
    rts

; Track an unpatched placeholder operand until cc_patch/cc_patch_here resolves it.
; The bitset covers all 2048 possible byte positions without a second limit.
cc_track_patch:
    jsr cc_patch_bit_address
    ldy cc_bit_index
    lda patch_bits,y
    and cc_bit_mask
    bne @done
    lda patch_bits,y
    ora cc_bit_mask
    sta patch_bits,y
    inc cc_pending_count
    bne @done
    inc cc_pending_count+1
@done:
    rts

cc_untrack_patch:
    jsr cc_patch_bit_address
    ldy cc_bit_index
    lda patch_bits,y
    and cc_bit_mask
    beq @done
    lda cc_bit_mask
    eor #$ff
    and patch_bits,y
    sta patch_bits,y
    lda cc_pending_count
    bne @low
    dec cc_pending_count+1
@low:
    dec cc_pending_count
@done:
    rts

; A/X = operand low-byte address within the arena.
cc_patch_bit_address:
    sec
    sbc #<cc_arena
    sta cc_bit_index
    txa
    sbc #>cc_arena
    sta cc_bit_index+1
    lda cc_bit_index
    and #7
    tax
    lda patch_masks,x
    sta cc_bit_mask
    ldx #3
@divide:
    lsr cc_bit_index+1
    ror cc_bit_index
    dex
    bne @divide
    rts
