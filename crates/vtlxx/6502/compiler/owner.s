; Owner-local labels/fixups and completion of executable code targets.
.setcpu "6502"
.import session_backend_error

.export cc_init, cc_begin_owner, cc_complete_owner
.export cc_define_label, cc_label_jump, cc_completed_entry, cc_consume_completed, owner_require_active
.import cc_arena_reset, cc_patch_reset, cc_freeze, cc_mark, cc_return, cc_jump, cc_jump_placeholder, cc_patch
.import cc_binding_reset, cc_static_reset
.importzp cc_status, cc_arg, cc_pending_count

.importzp zp_phase
owner_state = zp_phase + 67              ; 0 idle/consumed, 1 emitting, 2 completed
owner_entry = zp_phase + 68
label_count = zp_phase + 70
fixup_count = zp_phase + 71
owner_word = zp_phase + 72
owner_target = zp_phase + 74
owner_index = zp_phase + 76

.segment "BSS"
label_values: .res 64
label_targets: .res 64
fixup_values: .res 64
fixup_patches: .res 64
fixup_active: .res 32

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
    jsr cc_static_reset
    rts

; A/X = unpublished entry address.
cc_begin_owner:
    lda owner_state
    cmp #1
    beq @misuse
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
    jmp session_backend_error

; Checks all unresolved references, appends the final RTS, then marks entry completed.
cc_complete_owner:
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
    jmp session_backend_error
@terminator:
    jsr cc_return
    jsr cc_freeze
    lda #2
    sta owner_state
@done:
    lda owner_entry
    ldx owner_entry+1
    rts
@misuse:
    lda #3
    jmp session_backend_error

; Return only the most recently completed, unconsumed owner entry.
cc_completed_entry:
    lda owner_state
    cmp #2
    beq @ready
    lda #8
    jmp session_backend_error
@ready:
    lda owner_entry
    ldx owner_entry+1
    rts

cc_consume_completed:
    lda #0
    sta owner_state
    rts

; A/X = resolved numeric label value.
cc_define_label:
    sta owner_word
    stx owner_word+1
    jsr owner_require_active
    jsr owner_find_label
    bcc @new
    lda #4
    jmp session_backend_error
@new:
    ldx label_count
    cpx #32
    bcc @room
    lda #5
    jmp session_backend_error
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
    jmp session_backend_error
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
    lda owner_state
    cmp #1
    beq @done
    lda #3
    jmp session_backend_error
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
