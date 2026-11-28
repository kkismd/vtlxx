; #275 backend proof. TEST_CASE selects one independent compiler operation sequence.
.setcpu "6502"
.macpack longbranch
.export _main
.import cc_init, cc_mark, cc_emit_byte, cc_emit_u16, cc_patch
.import cc_push_const, cc_call, cc_return, cc_jump, cc_jz
.import cc_load_reg, cc_store_reg
.import cc_byte_op, cc_byte_store_cell, cc_byte_operand
.importzp rt_regs, rt_stack
.import cc_jump_placeholder, cc_jz_placeholder, cc_patch_here, cc_jump_to
.import cc_begin_owner, cc_complete_owner, cc_is_bound, cc_publish, cc_resolve
.import cc_define_label, cc_label_jump
.import cc_static_begin, cc_static_byte, cc_static_finish, cc_static_read
.import cc_static_start, cc_static_length
.import cc_name_len, cc_name
.import cc_arena
.import rt_init, rt_print_number, rt_print_char, rt_push
.import serial_out, halt
.importzp cc_status, cc_arg, cc_cursor, cc_pending_count, rt_depth
.importzp session_entry_sp, fe_compile_status, sp

.macro WORD value
    lda #<(value)
    ldx #>(value)
.endmacro

; Set the private binding input for a single-character name.
.macro NAME value
    lda #1
    sta cc_name_len
    lda #value
    sta cc_name
.endmacro

.macro OK
    lda cc_status
    beq :+
    jmp bad
:
.endmacro

.macro ERROR
    lda cc_status
    bne :+
    jmp bad
:
.endmacro

.segment "ZEROPAGE"
probe_ptr:   .res 2
entry:       .res 2

.segment "BSS"
patch:       .res 2
patch_two:   .res 2
address:     .res 2
counter:     .res 2
call_count:  .res 1
caller_sp: .res 1
software_sp: .res 2
phase: .res 1
patches: .res 98
byte_op: .res 1
byte_mode: .res 1
byte_slot: .res 1
byte_expected: .res 1
byte_target: .res 1

.segment "CODE"
_main:
    jsr rt_init
    jsr cc_init
.if TEST_CASE = 5 .or TEST_CASE = 6 .or TEST_CASE = 19 .or TEST_CASE = 21 .or TEST_CASE = 22 .or TEST_CASE = 40 .or TEST_CASE = 62 .or TEST_CASE = 63 .or TEST_CASE = 65
    ; Detect even a single byte written beyond the arena.
    lda #$a5
    sta cc_arena+$6000
.endif
    lda #$a5
    pha
    tsx
    stx caller_sp
    lda sp
    sta software_sp
    lda sp+1
    sta software_sp+1
    jsr probe_session
.if TEST_CASE = 4 .or TEST_CASE = 5 .or TEST_CASE = 6 .or TEST_CASE = 8 .or TEST_CASE = 9 .or TEST_CASE = 11 .or TEST_CASE = 12 .or TEST_CASE = 13 .or TEST_CASE = 15 .or TEST_CASE = 16 .or TEST_CASE = 19 .or TEST_CASE = 20 .or TEST_CASE = 21 .or TEST_CASE = 22 .or TEST_CASE = 31 .or TEST_CASE = 32 .or TEST_CASE = 33 .or TEST_CASE = 34 .or TEST_CASE = 35 .or TEST_CASE = 36 .or TEST_CASE = 37 .or TEST_CASE = 38 .or TEST_CASE = 39 .or TEST_CASE = 40 .or TEST_CASE = 41 .or TEST_CASE = 42 .or TEST_CASE = 43 .or TEST_CASE = 44 .or TEST_CASE = 45 .or TEST_CASE = 46 .or TEST_CASE = 48 .or TEST_CASE = 49 .or TEST_CASE = 51 .or TEST_CASE = 52 .or TEST_CASE = 54 .or TEST_CASE = 55 .or TEST_CASE = 56 .or TEST_CASE = 57 .or TEST_CASE = 60 .or TEST_CASE = 61 .or TEST_CASE = 62 .or TEST_CASE = 63 .or TEST_CASE = 64 .or TEST_CASE = 66 .or TEST_CASE = 67 .or TEST_CASE = 68 .or TEST_CASE = 69 .or TEST_CASE = 70
    bcs :+
    jmp bad
:
    lda fe_compile_status
    cmp #4
    beq :+
    jmp bad
:
.if TEST_CASE = 5 .or TEST_CASE = 6 .or TEST_CASE = 19 .or TEST_CASE = 21 .or TEST_CASE = 22 .or TEST_CASE = 33 .or TEST_CASE = 40 .or TEST_CASE = 62 .or TEST_CASE = 63
    lda #1
.elseif TEST_CASE = 4 .or TEST_CASE = 16 .or TEST_CASE = 32 .or TEST_CASE = 48 .or TEST_CASE = 54 .or TEST_CASE = 55 .or TEST_CASE = 56 .or TEST_CASE = 57
    lda #2
.elseif TEST_CASE = 34 .or TEST_CASE = 52 .or TEST_CASE = 60 .or TEST_CASE = 61 .or TEST_CASE = 64 .or TEST_CASE = 66 .or TEST_CASE = 67 .or TEST_CASE = 68 .or TEST_CASE = 69 .or TEST_CASE = 70
    lda #3
.elseif TEST_CASE = 11
    lda #4
.elseif TEST_CASE = 12
    lda #5
.elseif TEST_CASE = 13
    lda #6
.elseif TEST_CASE = 15 .or TEST_CASE = 31 .or TEST_CASE = 38 .or TEST_CASE = 39 .or TEST_CASE = 41
    lda #7
.else
    lda #8
.endif
    cmp cc_status
    beq :+
    jmp bad
:
.if TEST_CASE = 4 .or TEST_CASE = 5 .or TEST_CASE = 6 .or TEST_CASE = 8 .or TEST_CASE = 15 .or TEST_CASE = 16 .or TEST_CASE = 19 .or TEST_CASE = 20 .or TEST_CASE = 21 .or TEST_CASE = 22 .or TEST_CASE = 31 .or TEST_CASE = 32 .or TEST_CASE = 33 .or TEST_CASE = 38 .or TEST_CASE = 40 .or TEST_CASE = 41 .or TEST_CASE = 48 .or TEST_CASE = 44 .or TEST_CASE = 45 .or TEST_CASE = 46 .or TEST_CASE = 49 .or TEST_CASE = 51 .or TEST_CASE = 52 .or TEST_CASE = 54 .or TEST_CASE = 55 .or TEST_CASE = 56 .or TEST_CASE = 57
    lda phase
    cmp #1
    jne bad
.endif
.else
    bcc :+
    jmp bad
:
.endif
.if TEST_CASE = 5 .or TEST_CASE = 6 .or TEST_CASE = 19 .or TEST_CASE = 21 .or TEST_CASE = 22 .or TEST_CASE = 33 .or TEST_CASE = 40 .or TEST_CASE = 62 .or TEST_CASE = 63 .or TEST_CASE = 65
    lda cc_arena+$6000
    cmp #$a5
    jne bad
.endif
.if TEST_CASE = 54
    lda cc_arena+$6062
    cmp #$a5
    jne bad
.endif
.if TEST_CASE = 61
    lda cc_arena
    cmp #$60               ; completed owner's RTS is unchanged
    jne bad
.elseif TEST_CASE = 62
    lda cc_arena+$5ffe
    cmp #$a5
    jne bad
    lda cc_arena+$5fff
    cmp #$a5
    jne bad
.elseif TEST_CASE = 63
    lda cc_arena+$5ffb
    cmp #$a5
    jne bad
    lda cc_arena+$5fff
    cmp #$a5
    jne bad
.endif
.if TEST_CASE = 62 .or TEST_CASE = 63
    NAME 'z'
    jsr cc_is_bound
    jcs bad                ; aborted owner was not published
.endif
    tsx
    cpx caller_sp
    jne bad
    lda sp
    cmp software_sp
    jne bad
    lda sp+1
    cmp software_sp+1
    jne bad
    pla
    cmp #$a5
    jne bad
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt

probe_session:
    tsx
    stx session_entry_sp
.if TEST_CASE = 1
    ; PushConst, direct Call to an already resolved target, and final Return.
    jsr cc_begin_owner
    sta entry
    stx entry+1
    WORD 42
    jsr cc_push_const
    OK
    WORD rt_print_number
    jsr cc_call
    OK
    jsr cc_return
    OK
    ; Explicit Return must bypass later bytes in the same owner.
    WORD '?'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    jsr cc_complete_owner
    OK
    jsr invoke
    lda rt_depth
    beq :+
    jmp bad
:
.elseif TEST_CASE = 2
    ; A forward placeholder enters later code, then a backward absolute Jump
    ; returns to an earlier block. The tail placeholder avoids looping.
    jsr cc_begin_owner
    sta entry
    stx entry+1
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    jsr cc_mark
    sta address
    stx address+1
    WORD 'B'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    jsr cc_jump_placeholder
    sta patch_two
    stx patch_two+1
    lda patch
    ldx patch+1
    jsr cc_patch_here
    lda address
    ldx address+1
    jsr cc_jump_to
    lda patch_two
    ldx patch_two+1
    jsr cc_patch_here
    jsr cc_complete_owner
    OK
    jsr invoke
.elseif TEST_CASE = 3
    ; JZ consumes exactly one Cell in both paths.
    jsr cc_begin_owner
    sta entry
    stx entry+1
    WORD 0
    jsr cc_push_const
    jsr cc_jz_placeholder
    sta patch
    stx patch+1
    WORD '?'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    lda patch
    ldx patch+1
    jsr cc_patch_here
    WORD 'Z'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    WORD 7
    jsr cc_push_const
    jsr cc_jz_placeholder
    sta patch
    stx patch+1
    WORD 'N'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    lda patch
    ldx patch+1
    jsr cc_patch_here
    jsr cc_complete_owner
    OK
    jsr invoke
    lda rt_depth
    beq :+
    jmp bad
:
.elseif TEST_CASE = 4
    ; Patch operand is the low byte of a fully emitted absolute field.
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    OK
    WORD $1234
    sta cc_arg
    stx cc_arg+1
    lda patch
    ldx patch+1
    jsr cc_patch
    OK
    lda patch
    sta cc_arg
    lda patch+1
    sta cc_arg+1
    ldy #0
    lda (cc_arg),y
    cmp #$34
    beq :+
    jmp bad
:
    iny
    lda (cc_arg),y
    cmp #$12
    beq :+
    jmp bad
:
    jsr cc_mark
    ; Three emitted bytes: low operand address + 2 = current cursor.
    sec
    sbc patch
    cmp #2
    beq :+
    jmp bad
:
    txa
    sbc patch+1
    beq :+
    jmp bad
:
    ; A field beginning at the cursor is outside the emitted range.
    lda #1
    sta phase
    jsr cc_mark
    jsr cc_patch
.elseif TEST_CASE = 5
    ; Exactly 24576 bytes fit. The next byte aborts.
@fill:
    lda #$ea
    jsr cc_emit_byte
    OK
    inc counter
    bne :+
    inc counter+1
:
    lda counter+1
    cmp #$60
    bne @fill
    lda counter
    bne @fill
    jsr cc_mark
    sta address
    stx address+1
    lda #1
    sta phase
    lda #$ea
    jsr cc_emit_byte
.elseif TEST_CASE = 6
    ; A seven-byte PushConst fails with six bytes free.
@fill:
    lda #$ea
    jsr cc_emit_byte
    inc counter
    bne :+
    inc counter+1
:
    lda counter+1
    cmp #$5f
    bne @fill
    lda counter
    cmp #$fa                 ; 0x5ffa = 24570
    bne @fill
    OK
    jsr cc_mark
    sta address
    stx address+1
    lda #1
    sta phase
    WORD $abcd
    jsr cc_push_const
.elseif TEST_CASE = 7
    ; Cross a page while retaining little-endian order and cursor progress.
    lda #$ff
    sta cc_cursor
    lda #>cc_arena+1
    sta cc_cursor+1
    WORD $a1b2
    jsr cc_emit_u16
    OK
    lda cc_cursor
    cmp #1
    jne bad
    lda cc_cursor+1
    cmp #>cc_arena+2
    jne bad
    jsr cc_mark
    sec
    sbc #2
    sta cc_arg
    txa
    sbc #0
    sta cc_arg+1
    ldy #0
    lda (cc_arg),y
    cmp #$b2
    beq :+
    jmp bad
:
    iny
    lda (cc_arg),y
    cmp #$a1
    beq :+
    jmp bad
:
.elseif TEST_CASE = 8
    ; Distinct Write names resolve to their own completed targets.
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta address
    stx address+1
    OK
    NAME 'a'
    jsr cc_publish
    OK
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta patch
    stx patch+1
    OK
    NAME 'b'
    jsr cc_publish
    OK
    NAME 'a'
    jsr cc_resolve
    cmp address
    beq :+
    jmp bad
:
    cpx address+1
    beq :+
    jmp bad
:
    OK
    NAME 'b'
    jsr cc_resolve
    cmp patch
    jne bad
    cpx patch+1
    jne bad
    OK
    jsr cc_begin_owner
    jsr cc_complete_owner
    lda #1
    sta phase
    NAME 'a'
    jsr cc_publish
.elseif TEST_CASE = 9
    ; An unpublished owner cannot be bound, and an unbound slot fails resolve.
    jsr cc_begin_owner
    NAME 'a'
    jsr cc_publish
.elseif TEST_CASE = 10
    ; Forward label skips '?' and backward label repeats until helper returns 0.
    jsr cc_begin_owner
    sta entry
    stx entry+1
    WORD 1
    jsr cc_label_jump
    WORD '?'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    WORD 1
    jsr cc_define_label
    OK
    WORD 'L'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    WORD 2
    jsr cc_define_label
    WORD next_condition
    jsr cc_call
    jsr cc_jz_placeholder
    sta patch
    stx patch+1
    WORD 2
    jsr cc_label_jump
    lda patch
    ldx patch+1
    jsr cc_patch_here
    jsr cc_complete_owner
    OK
    jsr invoke
    lda call_count
    cmp #2
    beq :+
    jmp bad
:
    lda rt_depth
    beq :+
    jmp bad
:
.elseif TEST_CASE = 11
    jsr cc_begin_owner
    WORD 7
    jsr cc_define_label
    OK
    WORD 7
    jsr cc_define_label
.elseif TEST_CASE = 12
    jsr cc_begin_owner
    lda #1
    sta counter
@labels:
    lda counter
    ldx #0
    jsr cc_define_label
    OK
    inc counter
    lda counter
    cmp #33
    bne @labels
    WORD 33
    jsr cc_define_label
.elseif TEST_CASE = 13
    jsr cc_begin_owner
    lda #0
    sta counter
@fixups:
    WORD 7
    jsr cc_label_jump
    OK
    inc counter
    lda counter
    cmp #32
    bne @fixups
    WORD 7
    jsr cc_label_jump
.elseif TEST_CASE = 15
    ; Manual placeholders are also unresolved owner work until patched.
    jsr cc_begin_owner
    jsr cc_jump_placeholder
    OK
    lda #1
    sta phase
    jsr cc_complete_owner
.elseif TEST_CASE = 16
    ; A completed owner is frozen even though its bytes remain in the arena.
    jsr cc_begin_owner
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    jsr cc_patch_here
    jsr cc_complete_owner
    OK
    WORD $1234
    sta cc_arg
    stx cc_arg+1
    lda #1
    sta phase
    lda patch
    ldx patch+1
    jsr cc_patch
.elseif TEST_CASE = 17
    ; A resolved user binding is encoded as JSR abs16 in a later owner.
    jsr cc_begin_owner
    WORD 'Q'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    jsr cc_complete_owner
    OK
    NAME 'a'
    jsr cc_publish
    OK
    jsr cc_begin_owner
    sta entry
    stx entry+1
    NAME 'a'
    jsr cc_resolve
    jsr cc_call
    jsr cc_complete_owner
    OK
    jsr invoke
.elseif TEST_CASE = 18
    ; Both ends of the register index range execute load/store templates.
    jsr cc_begin_owner
    sta entry
    stx entry+1
    WORD 'A'
    jsr cc_push_const
    ldx #0
    jsr cc_store_reg
    OK
    WORD 'Z'
    jsr cc_push_const
    ldx #25
    jsr cc_store_reg
    OK
    ldx #0
    jsr cc_load_reg
    WORD rt_print_char
    jsr cc_call
    ldx #25
    jsr cc_load_reg
    WORD rt_print_char
    jsr cc_call
    jsr cc_complete_owner
    OK
    lda entry
    sta cc_arg
    lda entry+1
    sta cc_arg+1
    ldy #0
    lda (cc_arg),y
    cmp #$a9
    beq :+
    jmp bad
:
    jsr invoke
    lda rt_depth
    beq :+
    jmp bad
:
.elseif TEST_CASE = 19
    ; Five-byte register template cannot fit into four remaining bytes.
    lda #<(cc_arena+$5ffc)
    sta cc_cursor
    lda #>(cc_arena+$5ffc)
    sta cc_cursor+1
    lda #1
    sta phase
    ldx #25
    jsr cc_store_reg
.elseif TEST_CASE = 20
    ; Availability queries are non-destructive and isolated by complete name.
    NAME 'a'
    jsr cc_is_bound
    bcc :+
    jmp bad
:
    lda cc_status
    beq :+
    jmp bad
:
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta address
    stx address+1
    OK
    NAME 'a'
    jsr cc_publish
    OK
    NAME 'a'
    jsr cc_is_bound
    bcs :+
    jmp bad
:
    NAME 'a'
    jsr cc_is_bound
    bcs :+
    jmp bad
:
    lda cc_status
    beq :+
    jmp bad
:
    NAME 'b'
    jsr cc_is_bound
    bcc :+
    jmp bad
:
    NAME 'c'
    jsr cc_is_bound
    bcc :+
    jmp bad
:
    lda cc_status
    beq :+
    jmp bad
:
    ; The query leaves the published target usable and duplicate publish fails.
    NAME 'a'
    jsr cc_resolve
    cmp address
    beq :+
    jmp bad
:
    cpx address+1
    beq :+
    jmp bad
:
    jsr cc_begin_owner
    jsr cc_complete_owner
    lda #1
    sta phase
    NAME 'a'
    jsr cc_publish
.elseif TEST_CASE = 21
    ; A wrapping cursor is rejected before any write.
    lda #$ff
    sta cc_cursor
    sta cc_cursor+1
    lda #1
    sta phase
    lda #$42
    jsr cc_emit_byte
.elseif TEST_CASE = 22
    ; Completion cannot append its terminator beyond the arena.
    lda #<(cc_arena+$5fff)
    sta cc_cursor
    lda #>(cc_arena+$5fff)
    sta cc_cursor+1
    jsr cc_begin_owner
    lda #$ea
    jsr cc_emit_byte
    lda #1
    sta phase
    jsr cc_complete_owner
.elseif TEST_CASE = 28
    ; Two placeholders at identical offsets in different 2 KiB pages.
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    lda #<(cc_arena+$800)
    sta cc_cursor
    lda #>(cc_arena+$800)
    sta cc_cursor+1
    jsr cc_jump_placeholder
    sta patch_two
    stx patch_two+1
    lda cc_pending_count
    cmp #2
    beq :+
    jmp bad
:
    lda cc_pending_count+1
    beq :+
    jmp bad
:
    lda patch
    ldx patch+1
    jsr cc_patch_here
    OK
    lda cc_pending_count
    cmp #1
    beq :+
    jmp bad
:
    lda cc_pending_count+1
    beq :+
    jmp bad
:
    lda patch_two
    ldx patch_two+1
    jsr cc_patch_here
    OK
    lda cc_pending_count
    ora cc_pending_count+1
    beq :+
    jmp bad
:
    ; The last legal u16 operand starts at $5ffe.
    lda #<(cc_arena+$5ffd)
    sta cc_cursor
    lda #>(cc_arena+$5ffd)
    sta cc_cursor+1
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    OK
    lda cc_pending_count
    cmp #1
    beq :+
    jmp bad
:
    lda patch
    ldx patch+1
    jsr cc_patch_here
    OK
    lda cc_pending_count
    ora cc_pending_count+1
    beq :+
    jmp bad
:
.elseif TEST_CASE = 29
    jsr cc_begin_owner
    sta entry
    stx entry+1
    WORD 65
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    jsr cc_static_begin
    lda #$00              ; BRK must never execute
    jsr cc_static_byte
    lda #$ff
    jsr cc_static_byte
    jsr cc_static_finish
    OK
    lda cc_static_length
    cmp #2
    jne bad
    WORD 1
    jsr cc_push_const
    jsr cc_static_read
    WORD rt_print_number
    jsr cc_call
    jsr cc_static_begin   ; empty and terminal payloads
    jsr cc_static_finish
    jsr cc_static_begin
    lda #0
    jsr cc_static_byte
    jsr cc_static_finish
    jsr cc_complete_owner
    OK
    jsr rt_init
    jsr invoke
.elseif TEST_CASE = 30
    jsr cc_begin_owner
    jsr cc_static_begin
    ldx #0
@bytes:
    txa
    pha
    jsr cc_static_byte
    pla
    tax
    inx
    bne @bytes
    lda #42
    jsr cc_static_byte
    jsr cc_static_finish
    OK
    lda cc_static_length
    cmp #1
    jne bad
    lda cc_static_length+1
    cmp #1
    jne bad
    jsr cc_complete_owner
    OK
.elseif TEST_CASE = 31
    jsr cc_begin_owner
    jsr cc_static_begin
    lda #1
    jsr cc_static_byte
    lda #1
    sta phase
    jsr cc_complete_owner ; outstanding skip prevents completion
.elseif TEST_CASE = 32
    jsr cc_begin_owner
    jsr cc_static_begin
    lda #1
    jsr cc_static_byte
    lda #2
    jsr cc_static_byte
    jsr cc_static_finish
    OK
    lda #1
    sta phase
    lda cc_static_start
    ldx cc_static_start+1
    jsr cc_patch          ; payload is not a tracked operand
.elseif TEST_CASE = 33
    jsr cc_begin_owner
    lda #<(cc_arena+$5ffc)
    sta cc_cursor
    lda #>(cc_arena+$5ffc)
    sta cc_cursor+1
    jsr cc_static_begin
    lda #42
    jsr cc_static_byte    ; exactly fills the arena
    OK
    ; Registration is complete; use the first table byte as an arena sentinel.
    lda #$a5
    sta cc_arena+$6000
    lda #1
    sta phase
    lda #43
    jsr cc_static_byte
.elseif TEST_CASE = 34
    ; A static byte outside a payload is owner/static misuse.
    lda #42
    jsr cc_static_byte
.elseif TEST_CASE = 35
    NAME 'z'
    jsr cc_resolve
.elseif TEST_CASE = 36
    lda #0
    sta cc_name_len
    jsr cc_resolve
.elseif TEST_CASE = 37
    NAME 'A'
    jsr cc_resolve
.elseif TEST_CASE = 38
    jsr cc_begin_owner
    WORD 7
    jsr cc_label_jump
    lda #1
    sta phase
    jsr cc_complete_owner
.elseif TEST_CASE = 39
    ; Labels belong to the owner in which they were defined.
    jsr cc_begin_owner
    WORD 7
    jsr cc_define_label
    jsr cc_complete_owner
    jsr cc_begin_owner
    WORD 7
    jsr cc_label_jump
    jsr cc_complete_owner
.elseif TEST_CASE = 40
    lda #<(cc_arena+$5ffc)
    sta cc_cursor
    lda #>(cc_arena+$5ffc)
    sta cc_cursor+1
    lda #1
    sta phase
    ldx #25
    jsr cc_load_reg
.elseif TEST_CASE = 41
    jsr cc_begin_owner
    jsr cc_jz_placeholder
    lda #1
    sta phase
    jsr cc_complete_owner
.elseif TEST_CASE = 42
    lda #0
    sta cc_name_len
    jsr cc_is_bound
.elseif TEST_CASE = 43
    NAME 'A'
    jsr cc_is_bound
.elseif TEST_CASE = 44
    ; Reinitialization revokes an unconsumed completion.
    jsr cc_begin_owner
    jsr cc_complete_owner
    jsr cc_init
    lda #1
    sta phase
    NAME 'a'
    jsr cc_publish
.elseif TEST_CASE = 45
    ; Starting a new owner revokes the previous unpublished completion.
    jsr cc_begin_owner
    jsr cc_complete_owner
    jsr cc_begin_owner
    lda #1
    sta phase
    NAME 'a'
    jsr cc_publish
.elseif TEST_CASE = 46
    ; A different name cannot consume the same completion twice.
    jsr cc_begin_owner
    jsr cc_complete_owner
    NAME 'a'
    jsr cc_publish
    lda #1
    sta phase
    NAME 'b'
    jsr cc_publish
.elseif TEST_CASE = 47
    ; A new session starts with no binding from a successful previous one.
    jsr cc_begin_owner
    jsr cc_complete_owner
    NAME 'a'
    jsr cc_publish
    NAME 'a'
    jsr cc_is_bound
    bcs :+
    jmp bad
:
    jsr cc_init
    NAME 'a'
    jsr cc_is_bound
    bcc :+
    jmp bad
:
.elseif TEST_CASE = 48
    ; Resolving the same operand twice fails while its owner is still active.
    jsr cc_begin_owner
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    jsr cc_patch_here
    lda #1
    sta phase
    lda patch
    ldx patch+1
    jsr cc_patch_here
.elseif TEST_CASE = 49
    ; An initialized session has no completed owner to publish.
    lda #1
    sta phase
    NAME 'a'
    jsr cc_publish
.elseif TEST_CASE = 50
    ; Intermediate and final arena entries publish and resolve correctly.
    lda #<(cc_arena+$800)
    sta cc_cursor
    lda #>(cc_arena+$800)
    sta cc_cursor+1
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta address
    stx address+1
    cmp #<(cc_arena+$800)
    jne bad
    cpx #>(cc_arena+$800)
    jne bad
    NAME 'a'
    jsr cc_publish
    NAME 'a'
    jsr cc_resolve
    cmp address
    jne bad
    cpx address+1
    jne bad
    lda #<(cc_arena+$5fff)
    sta cc_cursor
    lda #>(cc_arena+$5fff)
    sta cc_cursor+1
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta address
    stx address+1
    cmp #<(cc_arena+$5fff)
    jne bad
    cpx #>(cc_arena+$5fff)
    jne bad
    NAME 'b'
    jsr cc_publish
    NAME 'b'
    jsr cc_resolve
    cmp address
    jne bad
    cpx address+1
    jne bad
    jsr cc_init
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta address
    stx address+1
    NAME 'c'
    jsr cc_publish
    NAME 'c'
    jsr cc_resolve
    cmp address
    jne bad
    cpx address+1
    jne bad
.elseif TEST_CASE = 51
    ; A consumed owner remains unavailable after another begins.
    jsr cc_begin_owner
    jsr cc_complete_owner
    NAME 'a'
    jsr cc_publish
    jsr cc_begin_owner
    lda #1
    sta phase
    NAME 'b'
    jsr cc_publish
.elseif TEST_CASE = 52
    ; Beginning another owner during construction is misuse.
    jsr cc_begin_owner
    lda #1
    sta phase
    jsr cc_begin_owner
 .elseif TEST_CASE = 53
    ; Fill all slots through real emitter calls, consume a middle slot, then
    ; reuse it. Reverse resolution checks swapped and surviving entries.
    jsr cc_begin_owner
    ldx #0
@register:
    stx counter
    jsr cc_jump_placeholder
    ldy counter
    sta patches,y
    txa
    sta patches+1,y
    ldx counter
    inx
    inx
    cpx #98
    bne @register
    lda cc_pending_count
    cmp #49
    jne bad
    lda cc_pending_count+1
    jne bad
    WORD $a15c
    sta cc_arg
    stx cc_arg+1
    lda patches+20
    ldx patches+21
    jsr cc_patch
    ldy #20
    jsr verify_value
    lda cc_pending_count
    cmp #48
    jne bad
    jsr cc_jump_placeholder
    sta patches+20
    stx patches+21
    lda cc_pending_count
    cmp #49
    jne bad
    ldx #96
@resolve:
    stx counter
    WORD $a15c
    sta cc_arg
    stx cc_arg+1
    ldy counter
    lda patches,y
    ldx patches+1,y
    jsr cc_patch
    ldy counter
    jsr verify_value
    ldx counter
    dex
    dex
    bpl @resolve
    lda cc_pending_count
    ora cc_pending_count+1
    jne bad
    ; A fresh session discards pending registrations.
    jsr cc_jump_placeholder
    jsr cc_init
    lda cc_pending_count
    ora cc_pending_count+1
    jne bad
.elseif TEST_CASE = 54
    ; A 50th real placeholder must abort after the first 49 succeeded.
    jsr cc_begin_owner
    lda #0
    sta counter
@fill_table:
    jsr cc_jump_placeholder
    inc counter
    lda counter
    cmp #49
    bne @fill_table
    lda cc_pending_count
    cmp #49
    jne bad
    lda cc_pending_count+1
    jne bad
    lda #$a5
    sta cc_arena+$6062
    lda #1
    sta phase
    jsr cc_jump_placeholder
.elseif TEST_CASE = 55
    ; Session reset cannot retain an earlier registration.
    jsr cc_begin_owner
    jsr cc_jump_placeholder
    sta patch
    stx patch+1
    jsr cc_init
    jsr cc_begin_owner
    ; Emit equivalent bytes without registering a placeholder.
    WORD $1234
    jsr cc_jump
    lda #1
    sta phase
    lda patch
    ldx patch+1
    jsr cc_patch
.elseif TEST_CASE = 56
    ; Generated operands which were never placeholders are not patchable.
    jsr cc_begin_owner
    WORD $1234
    jsr cc_jump
    jsr cc_mark
    sec
    sbc #2
    sta patch
    txa
    sbc #0
    tax
    lda #1
    sta phase
    lda patch
    jsr cc_patch
.elseif TEST_CASE = 57
    ; Reject operand+2 wrap independently of prior operations.
    jsr cc_begin_owner
    lda #1
    sta phase
    WORD $ffff
    jsr cc_patch
 .elseif TEST_CASE = 58
    ; Exhaustive opcode/mode matrix: immediate, $00ff and $0100.
    jsr cc_begin_owner
    lda #0
    sta byte_op
    sta byte_slot
@op:
    lda #0
    sta byte_mode
@mode:
    ; STORE has no immediate form.
    lda byte_op
    cmp #1
    bne @emit
    lda byte_mode
    beq @next
@emit:
    lda cc_cursor
    sta probe_ptr
    lda cc_cursor+1
    sta probe_ptr+1
    lda #$ff
    sta cc_byte_operand
    lda #0
    sta cc_byte_operand+1
    ldx byte_mode
    beq @call
    dex
    beq :+
    lda #1
    sta cc_byte_operand+1
:
    ldx #1
@call:
    lda byte_op
    jsr cc_byte_op
    ldy #0
    lda byte_op
    cmp #5
    bcc @opcode
    beq :+
    lda #$38
    bne :++
:
    lda #$18
:
    cmp (probe_ptr),y
    jne bad
    iny
@opcode:
    ldx byte_slot
    lda byte_opcodes,x
    cmp (probe_ptr),y
    jne bad
    iny
    lda (probe_ptr),y
    cmp #$ff
    jne bad
    lda byte_mode
    cmp #2
    bne @next
    iny
    lda (probe_ptr),y
    cmp #1
    jne bad
@next:
    inc byte_slot
    inc byte_mode
    lda byte_mode
    cmp #3
    jne @mode
    inc byte_op
    lda byte_op
    cmp #7
    jne @op
    ; Every Cell index and selector maps to the runtime zero page.
    lda #0
    sta byte_op
@cell:
    lda #0
    sta byte_mode
@selector:
    lda cc_cursor
    sta probe_ptr
    lda cc_cursor+1
    sta probe_ptr+1
    lda byte_op
    sta cc_byte_operand
    lda byte_mode
    sta cc_byte_operand+1
    lda #0
    ldx #2
    jsr cc_byte_op
    ldy #0
    lda (probe_ptr),y
    cmp #$a5
    jne bad
    iny
    lda byte_op
    asl a
    clc
    adc #<rt_regs
    adc byte_mode
    cmp (probe_ptr),y
    jne bad
    inc byte_mode
    lda byte_mode
    cmp #2
    bne @selector
    lda cc_cursor
    sta probe_ptr
    lda cc_cursor+1
    sta probe_ptr+1
    lda byte_op
    jsr cc_byte_store_cell
    ldy #0
    lda (probe_ptr),y
    cmp #$85
    jne bad
    iny
    lda byte_op
    asl a
    clc
    adc #<rt_regs
    cmp (probe_ptr),y
    jne bad
    iny
    lda (probe_ptr),y
    cmp #$a2
    jne bad
    iny
    lda (probe_ptr),y
    jne bad
    iny
    lda (probe_ptr),y
    cmp #$86
    jne bad
    iny
    lda byte_op
    asl a
    clc
    adc #<rt_regs+1
    cmp (probe_ptr),y
    jne bad
    inc byte_op
    lda byte_op
    cmp #26
    jne @cell
    ; Full-address upper boundary is an absolute operand.
    lda cc_cursor
    sta probe_ptr
    lda cc_cursor+1
    sta probe_ptr+1
    lda #$ff
    sta cc_byte_operand
    sta cc_byte_operand+1
    lda #0
    ldx #1
    jsr cc_byte_op
    ldy #0
    lda (probe_ptr),y
    cmp #$ad
    jne bad
    iny
    lda (probe_ptr),y
    cmp #$ff
    jne bad
    iny
    lda (probe_ptr),y
    cmp #$ff
    jne bad
.elseif TEST_CASE = 59
    ; Execute only after completion and reset runtime overlay after compilation.
    jsr cc_begin_owner
    sta entry
    stx entry+1
    lda #$ff
    sta cc_byte_operand
    lda #0
    ldx #0
    jsr cc_byte_op           ; A = 255
    lda #1
    sta cc_byte_operand
    lda #5
    ldx #0
    jsr cc_byte_op           ; A = 0 regardless of incoming carry
    lda #1
    sta cc_byte_operand
    lda #6
    ldx #0
    jsr cc_byte_op           ; A = 255, carry clear
    lda #0
    sta cc_byte_operand
    lda #6
    ldx #0
    jsr cc_byte_op           ; A remains 255 even with incoming carry clear
    lda #$0f
    sta cc_byte_operand
    lda #2
    ldx #0
    jsr cc_byte_op
    lda #$30
    sta cc_byte_operand
    lda #3
    ldx #0
    jsr cc_byte_op
    lda #$03
    sta cc_byte_operand
    lda #4
    ldx #0
    jsr cc_byte_op           ; A = $3c
    lda #25
    jsr cc_byte_store_cell
    lda #<byte_target
    sta cc_byte_operand
    lda #>byte_target
    sta cc_byte_operand+1
    lda #1
    ldx #1
    jsr cc_byte_op
    lda #0
    ldx #1
    jsr cc_byte_op           ; read back test-owned RAM
    lda #<probe_ptr
    sta cc_byte_operand
    lda #0
    sta cc_byte_operand+1
    lda #1
    ldx #1
    jsr cc_byte_op           ; store to test-owned zero page
    lda #0
    ldx #1
    jsr cc_byte_op           ; load from test-owned zero page
    lda #0
    sta cc_byte_operand
    sta cc_byte_operand+1
    lda #4
    ldx #2
    jsr cc_byte_op           ; XOR Cell A low
    lda #<byte_target
    sta cc_byte_operand
    lda #>byte_target
    sta cc_byte_operand+1
    lda #1
    ldx #1
    jsr cc_byte_op           ; A and RAM now $3f
    lda #25
    sta cc_byte_operand
    lda #0
    sta cc_byte_operand+1
    ldx #2
    jsr cc_byte_op           ; read stored low byte
    jsr cc_complete_owner
    jsr rt_init
    lda #$55
    sta rt_stack
    lda #3
    sta rt_regs
    lda #$aa
    sta rt_regs+49
    sta rt_regs+51
    lda #$42
    sec
    jsr invoke
    cmp #$3c
    jne bad
    cmp rt_regs+50
    jne bad
    lda byte_target
    cmp #$3f
    jne bad
    lda probe_ptr
    cmp #$3c
    jne bad
    lda rt_regs+51
    jne bad
    lda rt_regs+49
    cmp #$aa
    jne bad
    lda rt_stack
    cmp #$55
    jne bad
.elseif TEST_CASE = 60
    ; No active owner.
    lda #0
    ldx #0
    jsr cc_byte_op
.elseif TEST_CASE = 61
    ; Completed owner is frozen.
    jsr cc_begin_owner
    jsr cc_complete_owner
    lda #0
    jsr cc_byte_store_cell
.elseif TEST_CASE = 62
    ; ADD immediate is three bytes: two free cannot fit prefix and operand.
    lda #$a5
    sta cc_arena+$5ffe
    sta cc_arena+$5fff
    jsr cc_begin_owner
    lda #<(cc_arena+$5ffe)
    sta cc_cursor
    lda #>(cc_arena+$5ffe)
    sta cc_cursor+1
    lda #5
    ldx #0
    jsr cc_byte_op
.elseif TEST_CASE = 63
    ; Cell store reserves all six bytes together.
    lda #$a5
    sta cc_arena+$5ffb
    sta cc_arena+$5fff
    jsr cc_begin_owner
    lda #<(cc_arena+$5ffb)
    sta cc_cursor
    lda #>(cc_arena+$5ffb)
    sta cc_cursor+1
    lda #25
    jsr cc_byte_store_cell
.elseif TEST_CASE = 64
    ; An invalid internal Cell index must fail rather than address outside Cells.
    jsr cc_begin_owner
    lda #26
    jsr cc_byte_store_cell
 .elseif TEST_CASE = 65
    ; Both templates fit exactly, without writing the byte after the arena.
    jsr cc_begin_owner
    lda #<(cc_arena+$5ffa)
    sta cc_cursor
    lda #>(cc_arena+$5ffa)
    sta cc_cursor+1
    lda #25
    jsr cc_byte_store_cell
    lda cc_cursor
    cmp #<(cc_arena+$6000)
    jne bad
    lda cc_cursor+1
    cmp #>(cc_arena+$6000)
    jne bad
    jsr cc_init
    jsr cc_begin_owner
    lda #<(cc_arena+$5ffd)
    sta cc_cursor
    lda #>(cc_arena+$5ffd)
    sta cc_cursor+1
    lda #5
    ldx #0
    jsr cc_byte_op
    lda cc_cursor
    cmp #<(cc_arena+$6000)
    jne bad
    lda cc_cursor+1
    cmp #>(cc_arena+$6000)
    jne bad
.elseif TEST_CASE = 66
    jsr cc_begin_owner
    lda #0
    sta cc_byte_operand
    lda #2
    sta cc_byte_operand+1
    lda #0
    ldx #2
    jsr cc_byte_op
.elseif TEST_CASE = 67
    jsr cc_begin_owner
    lda #7
    ldx #0
    jsr cc_byte_op
.elseif TEST_CASE = 68
    jsr cc_begin_owner
    lda #1
    ldx #0
    jsr cc_byte_op
 .elseif TEST_CASE = 69
    jsr cc_begin_owner
    lda #26
    sta cc_byte_operand
    lda #0
    sta cc_byte_operand+1
    ldx #2
    jsr cc_byte_op
.elseif TEST_CASE = 70
    jsr cc_begin_owner
    lda #0
    ldx #3
    jsr cc_byte_op
.else
    .error "unknown backend probe case"
.endif
    clc
    rts

bad:
    lda #'F'
    jsr serial_out
    lda #9
    jmp halt

invoke:
    jmp (entry)

next_condition:
    inc call_count
    lda call_count
    cmp #1
    beq :+
    lda #0
    ldx #0
    jmp rt_push
:
    lda #1
    ldx #0
    jmp rt_push

; Y = byte offset of a saved operand. Check both bytes of the replacement.
verify_value:
    lda patches,y
    sta probe_ptr
    lda patches+1,y
    sta probe_ptr+1
    ldy #0
    lda (probe_ptr),y
    cmp #$5c
    jne bad
    iny
    lda (probe_ptr),y
    cmp #$a1
    jne bad
    rts

.segment "RODATA"
byte_opcodes:
    .byte $a9,$a5,$ad, $00,$85,$8d, $29,$25,$2d
    .byte $09,$05,$0d, $49,$45,$4d, $69,$65,$6d, $e9,$e5,$ed
