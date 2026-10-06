; #275 backend proof. TEST_CASE selects one independent compiler operation sequence.
.setcpu "6502"
.export _main
.import cc_init, cc_mark, cc_emit_byte, cc_emit_u16, cc_patch
.import cc_push_const, cc_call, cc_return, cc_jump, cc_jz
.import cc_jump_placeholder, cc_jz_placeholder, cc_patch_here, cc_jump_to
.import cc_begin_owner, cc_complete_owner, cc_publish, cc_resolve
.import cc_define_label, cc_label_jump
.import cc_work_push, cc_work_pop, cc_work_swap, cc_work_reset
.import rt_init, rt_print_number, rt_print_char, rt_push
.import serial_out, halt
.importzp cc_status, cc_arg, rt_depth

.macro WORD value
    lda #<(value)
    ldx #>(value)
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

.segment "BSS"
entry:       .res 2
patch:       .res 2
patch_two:   .res 2
address:     .res 2
counter:     .res 2
call_count:  .res 1

.segment "CODE"
_main:
    jsr rt_init
    jsr cc_init
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
    jsr cc_mark
    jsr cc_patch
    ERROR
.elseif TEST_CASE = 5
    ; Exactly 2048 bytes fit. The next byte fails without moving the cursor.
@fill:
    lda #$ea
    jsr cc_emit_byte
    OK
    inc counter
    bne :+
    inc counter+1
:
    lda counter+1
    cmp #8
    bne @fill
    lda counter
    bne @fill
    jsr cc_mark
    sta address
    stx address+1
    lda #$ea
    jsr cc_emit_byte
    ERROR
    jsr cc_mark
    cmp address
    beq :+
    jmp bad
:
    cpx address+1
    beq :+
    jmp bad
:
.elseif TEST_CASE = 6
    ; A seven-byte PushConst cannot partially append with six bytes free.
@fill:
    lda #$ea
    jsr cc_emit_byte
    inc counter
    bne :+
    inc counter+1
:
    lda counter+1
    cmp #7
    bne @fill
    lda counter
    cmp #$fa                 ; 0x07fa = 2042
    bne @fill
    OK
    jsr cc_mark
    sta address
    stx address+1
    WORD $abcd
    jsr cc_push_const
    ERROR
    jsr cc_mark
    cmp address
    beq :+
    jmp bad
:
    cpx address+1
    beq :+
    jmp bad
:
.elseif TEST_CASE = 7
    ; Raw u16 append retains the target's little-endian byte order.
    WORD $a1b2
    jsr cc_emit_u16
    OK
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
    ; The two binding roles have separate direct slots for the same identity.
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta address
    stx address+1
    sta cc_arg
    stx cc_arg+1
    OK
    lda #0
    ldx #'a'
    jsr cc_publish
    OK
    jsr cc_begin_owner
    jsr cc_complete_owner
    sta cc_arg
    stx cc_arg+1
    OK
    lda #1
    ldx #'a'
    jsr cc_publish
    OK
    lda #0
    ldx #'a'
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
    lda #1
    ldx #'a'
    jsr cc_resolve
    cmp address
    bne :+
    cpx address+1
    bne :+
    jmp bad
:
    OK
    lda #0
    ldx #'a'
    jsr cc_publish
    ERROR
.elseif TEST_CASE = 9
    ; An unpublished owner cannot be bound, and an unbound slot fails resolve.
    jsr cc_begin_owner
    sta cc_arg
    stx cc_arg+1
    lda #0
    ldx #'a'
    jsr cc_publish
    ERROR
    jsr cc_init
    lda #0
    ldx #'z'
    jsr cc_resolve
    ERROR
    jsr cc_init
    lda #2
    ldx #'a'
    jsr cc_resolve
    ERROR
    jsr cc_init
    lda #0
    ldx #'A'
    jsr cc_resolve
    ERROR
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
    ERROR
    jsr cc_init
    jsr cc_begin_owner
    WORD 7
    jsr cc_label_jump
    jsr cc_complete_owner
    ERROR
    ; Owner-local tables must not carry a previous owner's label.
    jsr cc_init
    jsr cc_begin_owner
    WORD 7
    jsr cc_define_label
    jsr cc_complete_owner
    OK
    jsr cc_begin_owner
    WORD 7
    jsr cc_label_jump
    jsr cc_complete_owner
    ERROR
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
    ERROR
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
    ERROR
.elseif TEST_CASE = 14
    lda #0
    sta counter
@push:
    lda counter
    ldx #$a5
    jsr cc_work_push
    OK
    inc counter
    lda counter
    cmp #16
    bne @push
    WORD $ffff
    jsr cc_work_push
    ERROR
    jsr cc_init
    jsr cc_work_reset
    WORD $1234
    jsr cc_work_push
    WORD $5678
    jsr cc_work_push
    jsr cc_work_swap
    OK
    jsr cc_work_pop
    cmp #$34
    beq :+
    jmp bad
:
    cpx #$12
    beq :+
    jmp bad
:
    jsr cc_work_pop
    cmp #$78
    beq :+
    jmp bad
:
    cpx #$56
    beq :+
    jmp bad
:
    jsr cc_work_pop
    ERROR
.elseif TEST_CASE = 15
    ; Manual placeholders are also unresolved owner work until patched.
    jsr cc_begin_owner
    jsr cc_jump_placeholder
    OK
    jsr cc_complete_owner
    ERROR
    jsr cc_init
    jsr cc_begin_owner
    jsr cc_jz_placeholder
    OK
    jsr cc_complete_owner
    ERROR
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
    lda patch
    ldx patch+1
    jsr cc_patch
    ERROR
.elseif TEST_CASE = 17
    ; A resolved user binding is encoded as JSR abs16 in a later owner.
    jsr cc_begin_owner
    WORD 'Q'
    jsr cc_push_const
    WORD rt_print_char
    jsr cc_call
    jsr cc_complete_owner
    sta cc_arg
    stx cc_arg+1
    OK
    lda #0
    ldx #'a'
    jsr cc_publish
    OK
    jsr cc_begin_owner
    sta entry
    stx entry+1
    lda #0
    ldx #'a'
    jsr cc_resolve
    jsr cc_call
    jsr cc_complete_owner
    OK
    jsr invoke
.else
    .error "unknown backend probe case"
.endif
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt

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
