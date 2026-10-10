; Owner-local labels/fixups and completion of executable code targets.
.setcpu "6502"

.export cc_init, cc_begin_owner, cc_complete_owner
.export cc_define_label, cc_label_jump, cc_is_completed
.export cc_source_begin, cc_source_append, cc_source_finish, cc_source_abort, cc_source_is_active
.import cc_arena_reset, cc_patch_reset, cc_freeze, cc_mark, cc_return, cc_jump, cc_jump_placeholder, cc_patch
.import cc_data_begin, cc_data_append, cc_data_finish, cc_data_abort
.import cc_data_is_active, cc_cursor_is_frozen
.import cc_binding_reset, cc_work_reset
.import cc_arena
.importzp cc_status, cc_arg, cc_pending_count, cc_data_length

.segment "ZEROPAGE"
owner_state: .res 1             ; 0 idle, 1 emitting
owner_entry: .res 2
label_count: .res 1
fixup_count: .res 1
owner_word: .res 2
owner_target: .res 2
owner_index: .res 1
owner_offset: .res 2
owner_mask: .res 1

.segment "BSS"
label_values: .res 64
label_targets: .res 64
fixup_values: .res 64
fixup_patches: .res 64
fixup_active: .res 32
completed_bits: .res 256       ; one bit per possible arena entry byte

.segment "RODATA"
bit_masks: .byte 1,2,4,8,16,32,64,128

.segment "CODE"
cc_init:
    lda #0
    sta cc_status
    sta owner_state
    sta label_count
    sta fixup_count
    jsr cc_arena_reset
    jsr cc_patch_reset
    jsr cc_binding_reset
    jsr cc_work_reset
    lda #0
    tax
@clear:
    sta completed_bits,x
    inx
    bne @clear
    rts

; A/X = unpublished entry address.
cc_begin_owner:
    lda cc_status
    bne @done
    lda owner_state
    bne @misuse
    jsr cc_data_is_active
    bne @misuse
    lda #1
    sta owner_state
    lda #0
    sta label_count
    sta fixup_count
    ldx #31
@clear_fixups:
    sta fixup_active,x
    dex
    bpl @clear_fixups
    jsr cc_mark
    sta owner_entry
    stx owner_entry+1
@done:
    lda owner_entry
    ldx owner_entry+1
    rts
@misuse:
    lda #3
    sta cc_status
    jmp @done

; Source-storage ABI: cc_status is the result (0 success, 1 capacity, 3 misuse).
; begin takes no arguments and clobbers A; it requires an idle owner at frozen
; code end with no pending patches. The arena owns all storage state.
cc_source_begin:
    lda cc_status
    bne @done
    lda owner_state
    bne @misuse
    jsr cc_data_is_active
    bne @misuse
    jsr cc_cursor_is_frozen
    cmp #1
    bne @misuse
    lda cc_pending_count
    ora cc_pending_count+1
    bne @misuse
    jmp cc_data_begin
@misuse:
    lda #3
    sta cc_status
@done:
    rts

; A = source byte; cc_status reports success or failure. A/Y are clobbered.
cc_source_append:
    jmp cc_data_append

; On success returns source start B in A/X and length L in cc_data_length.
; A/X/Y are clobbered; cc_status reports success or failure.
cc_source_finish:
    jmp cc_data_finish

; No arguments; active abort restores the temporary cursor and preserves status.
; Inactive abort is misuse and sets status 3.
cc_source_abort:
    jmp cc_data_abort

; Read-only query: A=0 inactive or A=1 active; cc_status is unchanged.
cc_source_is_active:
    jmp cc_data_is_active

; Checks all unresolved references, appends the final RTS, then marks entry completed.
cc_complete_owner:
    lda cc_status
    bne @done
    lda owner_state
    cmp #1
    bne @misuse
    lda fixup_count
    bne @unresolved
    lda cc_pending_count
    ora cc_pending_count+1
    beq @terminator
@unresolved:
    lda #7
    sta cc_status
    jmp @done
@terminator:
    jsr cc_return
    lda cc_status
    bne @done
    jsr cc_freeze
    lda owner_entry
    ldx owner_entry+1
    jsr owner_bit_address
    ldy owner_offset
    lda completed_bits,y
    ora owner_mask
    sta completed_bits,y
    lda #0
    sta owner_state
@done:
    lda owner_entry
    ldx owner_entry+1
    rts
@misuse:
    lda #3
    sta cc_status
    jmp @done

; A/X must be an entry produced by successful owner completion.
cc_is_completed:
    sta owner_target
    stx owner_target+1
    lda cc_status
    bne @done
    sec
    lda owner_target
    sbc #<cc_arena
    sta owner_offset
    lda owner_target+1
    sbc #>cc_arena
    sta owner_offset+1
    bcc @bad
    lda owner_offset+1
    cmp #8
    bcs @bad
    lda owner_target
    ldx owner_target+1
    jsr owner_bit_address
    ldy owner_offset
    lda completed_bits,y
    and owner_mask
    bne @done
@bad:
    lda #8
    sta cc_status
@done:
    rts

; Converts an in-arena address into byte index and bit mask.
owner_bit_address:
    sec
    sbc #<cc_arena
    sta owner_offset
    txa
    sbc #>cc_arena
    sta owner_offset+1
    lda owner_offset
    and #7
    tax
    lda bit_masks,x
    sta owner_mask
    ldx #3
@divide:
    lsr owner_offset+1
    ror owner_offset
    dex
    bne @divide
    rts

; A/X = resolved numeric label value.
cc_define_label:
    sta owner_word
    stx owner_word+1
    jsr owner_require_active
    lda cc_status
    bne @done
    jsr owner_find_label
    bcc @new
    lda #4
    sta cc_status
    rts
@new:
    ldx label_count
    cpx #32
    bcc @room
    lda #5
    sta cc_status
    rts
@room:
    txa
    asl a
    tay
    lda owner_word
    sta label_values,y
    lda owner_word+1
    sta label_values+1,y
    jsr cc_mark
    sta label_targets,y
    sta owner_target
    txa
    sta label_targets+1,y
    sta owner_target+1
    inc label_count
    lda #0
    sta owner_index
@fixups:
    ldx owner_index
    cpx #32
    beq @done
    lda fixup_active,x
    beq @next
    txa
    asl a
    tay
    lda fixup_values,y
    cmp owner_word
    bne @next
    lda fixup_values+1,y
    cmp owner_word+1
    bne @next
    lda owner_target
    sta cc_arg
    lda owner_target+1
    sta cc_arg+1
    lda fixup_patches,y
    pha
    lda fixup_patches+1,y
    tax
    pla
    jsr cc_patch
    lda cc_status
    bne @done
    ldx owner_index
    lda #0
    sta fixup_active,x
    dec fixup_count
@next:
    inc owner_index
    jmp @fixups
@done:
    rts

; A/X = numeric label value. Backward jumps resolve immediately.
cc_label_jump:
    sta owner_word
    stx owner_word+1
    jsr owner_require_active
    lda cc_status
    bne @done
    jsr owner_find_label
    bcc @forward
    lda label_targets,y
    pha
    lda label_targets+1,y
    tax
    pla
    jmp cc_jump
@forward:
    lda fixup_count
    cmp #32
    bcc @find_slot
    lda #6
    sta cc_status
    rts
@find_slot:
    ldx #0
@slot:
    lda fixup_active,x
    beq @found_slot
    inx
    cpx #32
    bne @slot
@found_slot:
    stx owner_index
    jsr cc_jump_placeholder
    sta owner_target
    stx owner_target+1
    lda cc_status
    bne @done
    ldx owner_index
    lda #1
    sta fixup_active,x
    txa
    asl a
    tay
    lda owner_word
    sta fixup_values,y
    lda owner_word+1
    sta fixup_values+1,y
    lda owner_target
    sta fixup_patches,y
    lda owner_target+1
    sta fixup_patches+1,y
    inc fixup_count
@done:
    rts

owner_require_active:
    lda cc_status
    bne @done
    lda owner_state
    cmp #1
    beq @done
    lda #3
    sta cc_status
@done:
    rts

; Carry set when found; Y is byte offset in the u16 arrays.
owner_find_label:
    ldx #0
@scan:
    cpx label_count
    beq @missing
    txa
    asl a
    tay
    lda label_values,y
    cmp owner_word
    bne @next
    lda label_values+1,y
    cmp owner_word+1
    beq @found
@next:
    inx
    bne @scan
@missing:
    clc
    rts
@found:
    sec
    rts
