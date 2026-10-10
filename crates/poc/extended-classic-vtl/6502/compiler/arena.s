; EC07 compiler-private, append-only 2 KiB native code arena.
.setcpu "6502"

.export cc_arena_reset, cc_mark, cc_emit_byte, cc_emit_u16, cc_patch
.export cc_patch_reset, cc_track_patch, cc_freeze
.export cc_reserve, cc_write_byte_raw
.export cc_data_begin, cc_data_append, cc_data_finish, cc_data_abort
.export cc_data_is_active, cc_cursor_is_frozen
.export cc_arena
.exportzp cc_status, cc_arg, cc_cursor, cc_pending_count, cc_data_length

.segment "ZEROPAGE"
cc_status: .res 1
cc_arg:    .res 2
cc_cursor: .res 2
cc_frozen_end: .res 2
cc_ptr:    .res 2
cc_limit:  .res 2
cc_value:  .res 2
cc_pending_count: .res 2
cc_source_begin: .res 2
cc_data_begin_ptr: .res 2
cc_data_active: .res 1
cc_data_length: .res 2
cc_reserve_count: .res 1
cc_data_value: .res 1
cc_copy_src: .res 2
cc_copy_dst: .res 2
cc_copy_count: .res 2
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
    lda #<cc_arena_end
    sta cc_source_begin
    lda #>cc_arena_end
    sta cc_source_begin+1
    lda #0
    sta cc_data_active
    sta cc_data_length
    sta cc_data_length+1
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

; Backend queries used by the owner gate; raw arena state stays private.
cc_data_is_active:
    lda cc_data_active
    rts

cc_cursor_is_frozen:
    lda cc_cursor
    cmp cc_frozen_end
    bne @different
    lda cc_cursor+1
    cmp cc_frozen_end+1
    bne @different
    lda #1
    rts
@different:
    lda #0
    rts

; A = required code byte count (1..255). No cursor mutation on failure.
cc_reserve:
    sta cc_reserve_count
    lda cc_status
    bne @done
    lda cc_data_active
    beq @ordinary
    lda #3
    sta cc_status
@done:
    rts
@ordinary:
    lda cc_reserve_count
    jmp cc_reserve_shared

; The data save path shares the same overflow-safe capacity check.
cc_reserve_shared:
    sta cc_reserve_count
    lda cc_status
    bne @done
    clc
    lda cc_cursor
    adc cc_reserve_count
    sta cc_limit
    lda cc_cursor+1
    adc #0
    bcs @full
    sta cc_limit+1
    cmp cc_source_begin+1
    bcc @ok
    bne @full
    lda cc_limit
    cmp cc_source_begin
    bcc @ok
    beq @ok
@full:
    lda #1
    sta cc_status
@ok:
@done:
    rts

; Caller must reserve the whole template first. A = byte to append.
cc_write_byte_raw:
    sta cc_data_value
    lda cc_data_active
    beq @write
    lda #3
    sta cc_status
    rts
@write:
    ; Recover the input byte from A via a private scratch before pointer use.
    ; Callers reserve before entering this internal writer.
    ldy #0
    lda cc_data_value
    sta (cc_cursor),y
    inc cc_cursor
    bne @done
    inc cc_cursor+1
@done:
    rts

; Begin a temporary forward append at the current code cursor.
cc_data_begin:
    lda cc_status
    bne @done
    lda cc_data_active
    bne @misuse
    lda cc_cursor
    sta cc_data_begin_ptr
    lda cc_cursor+1
    sta cc_data_begin_ptr+1
    lda #1
    sta cc_data_active
@done:
    rts
@misuse:
    lda #3
    sta cc_status
    rts

; A = one source byte. It is written only after the shared capacity check.
cc_data_append:
    sta cc_data_value
    lda cc_status
    bne @done
    lda cc_data_active
    bne @active
    lda #3
    sta cc_status
    rts
@active:
    lda #1
    jsr cc_reserve_shared
    lda cc_status
    bne @done
    lda cc_data_value
    ; This is the same raw cursor write, with data-active intentionally allowed.
    ldy #0
    sta (cc_cursor),y
    inc cc_cursor
    bne @done
    inc cc_cursor+1
@done:
    rts

; Finish and compact the new bytes against the previous source tail.
; Returns B in A/X and length in cc_data_length. No state publishes on failure.
cc_data_finish:
    lda cc_status
    beq :+
    rts
:
    lda cc_data_active
    bne @active
    jmp @misuse
@active:
    sec
    lda cc_cursor
    sbc cc_data_begin_ptr
    sta cc_data_length
    lda cc_cursor+1
    sbc cc_data_begin_ptr+1
    sta cc_data_length+1
    lda cc_data_length
    ora cc_data_length+1
    bne @nonempty
    jmp @misuse
@nonempty:
    sec
    lda cc_source_begin
    sbc cc_data_length
    sta cc_copy_dst
    lda cc_source_begin+1
    sbc cc_data_length+1
    sta cc_copy_dst+1
    ; A <= B is required so completed code and the new source do not overlap.
    lda cc_data_begin_ptr+1
    cmp cc_copy_dst+1
    bcc @range_ok
    bne @range_bad
    lda cc_data_begin_ptr
    cmp cc_copy_dst
    bcc @range_ok
    beq @range_ok
@range_bad:
    lda #1
    sta cc_status
    rts
@range_ok:
    clc
    lda cc_data_begin_ptr
    adc cc_data_length
    sta cc_copy_src
    lda cc_data_begin_ptr+1
    adc cc_data_length+1
    sta cc_copy_src+1
    clc
    lda cc_copy_dst
    adc cc_data_length
    sta cc_copy_dst
    lda cc_copy_dst+1
    adc cc_data_length+1
    sta cc_copy_dst+1
    lda cc_data_length
    sta cc_copy_count
    lda cc_data_length+1
    sta cc_copy_count+1
@copy:
    sec
    lda cc_copy_src
    sbc #1
    sta cc_copy_src
    lda cc_copy_src+1
    sbc #0
    sta cc_copy_src+1
    sec
    lda cc_copy_dst
    sbc #1
    sta cc_copy_dst
    lda cc_copy_dst+1
    sbc #0
    sta cc_copy_dst+1
    ldy #0
    lda (cc_copy_src),y
    sta (cc_copy_dst),y
    lda cc_copy_count
    bne @count_low
    dec cc_copy_count+1
@count_low:
    dec cc_copy_count
    lda cc_copy_count
    ora cc_copy_count+1
    bne @copy
    lda cc_copy_dst
    sta cc_source_begin
    lda cc_copy_dst+1
    sta cc_source_begin+1
    lda cc_data_begin_ptr
    sta cc_cursor
    lda cc_data_begin_ptr+1
    sta cc_cursor+1
    lda #0
    sta cc_data_active
    lda cc_source_begin
    ldx cc_source_begin+1
@done:
    rts
@misuse:
    lda #3
    sta cc_status
    rts

; Discard the current temporary append while preserving prior source bytes/status.
cc_data_abort:
    lda cc_data_active
    bne @active
    lda #3
    sta cc_status
    rts
@active:
    lda cc_data_begin_ptr
    sta cc_cursor
    lda cc_data_begin_ptr+1
    sta cc_cursor+1
    lda #0
    sta cc_data_active
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
