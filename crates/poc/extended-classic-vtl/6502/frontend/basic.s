; Streaming basic ECVTL source compiler and top-level compile/run driver.
; The only source state is one lookahead byte; no source text is retained.
.setcpu "6502"
.macpack longbranch

.export fe_compile_run
.exportzp fe_compile_status
; Private frontend ABI: identity in A; publish/lookup use cc_arg for start and
; cc_data_length for length. template_is_bound and lookup leave cc_status alone.
.import template_reset, template_is_bound, template_publish
.import cc_source_begin, cc_source_append, cc_source_finish, cc_source_abort, cc_source_is_active
.import fe_init, fe_next, cc_init, cc_begin_owner, cc_complete_owner
.import cc_define_label, cc_label_jump
.import cc_mark, cc_jz_placeholder, cc_jump_placeholder, cc_patch_here
.import cc_jump_to
.import cc_work_push, cc_work_pop
.import cc_is_bound, cc_publish, cc_resolve
.import cc_push_const, cc_call, cc_load_reg, cc_store_reg
.import rt_init, rt_load_storage, rt_store_storage
.import rt_add, rt_sub, rt_mul, rt_div, rt_rem
.import rt_eq, rt_ne, rt_lt, rt_le, rt_gt, rt_ge
.import rt_print_number, rt_print_char
.importzp fe_status, cc_status, cc_arg
.importzp cc_data_length

.segment "ZEROPAGE"
fe_compile_status: .res 1       ; 0 success, 1 source/transport, 2 syntax, 3 literal, 4 compile/backend resource
look_state:        .res 1       ; 0 empty, 1 byte, 2 frame end, 3 transport error
look_byte:         .res 1
target_index:      .res 1
nesting:           .res 1
structured_depth:  .res 1
while_depth:       .res 1
while_slot:        .res 1
while_offset:      .res 1
patch_scratch:     .res 2
while_word:        .res 2
negative:          .res 1
literal:           .res 2
digit:             .res 1
operator_index:    .res 1
entry:             .res 2
chunk_count:       .res 1
chunk_index:       .res 1
chunk_target:      .res 2
definition_identity: .res 1
call_target:       .res 2
constant_count:    .res 1
constant_length:   .res 1
constant_slot:     .res 1
constant_index:    .res 1
constant_char:     .res 1
constant_ptr:      .res 2
template_char:     .res 1
template_first:    .res 1
template_seen_hole: .res 1
template_statement_count: .res 1
template_saved_start: .res 2

.segment "RODATA"
operator_chars: .byte '+','-','*','/','%','<','>'
operator_low:   .byte <rt_add,<rt_sub,<rt_mul,<rt_div,<rt_rem,<rt_lt,<rt_gt
operator_high:  .byte >rt_add,>rt_sub,>rt_mul,>rt_div,>rt_rem,>rt_lt,>rt_gt

.segment "BSS"
constant_name: .res 16
constant_table: .res 16*19 ; 16-byte NUL-terminated name + signed i16
while_loop_targets: .res 16*2
while_exit_patches: .res 16*2
top_level_chunk_targets: .res 53*2

.segment "CODE"
; Carry clear on success; A/X is the completed entry. Compile errors never
; enter generated code. The caller may inspect fe_compile_status/cc_status.
fe_compile_run:
    lda #0
    sta fe_compile_status
    sta look_state
    sta nesting
    sta structured_depth
    sta while_depth
    sta chunk_count
    sta constant_count
    jsr fe_init
    jcs source_error
    jsr cc_init
    jsr template_reset
    jsr fe_start_chunk
    lda fe_compile_status
    bne @failed
@next_statement:
    jsr skip_separators
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    beq @complete
    jsr statement
    lda fe_compile_status
    bne @failed
    lda cc_status
    jne backend_error
    jmp @next_statement
@complete:
    ; Close and retain the final source chunk, then make the final driver the
    ; only runtime entry. No chunk executes until the whole source compiles.
    jsr fe_finish_chunk
    lda fe_compile_status
    bne @failed
    jsr fe_build_driver
    lda fe_compile_status
    bne @failed
    lda entry
    ldx entry+1
    jsr rt_init
    ; JSR through a completed entry, preserving a normal return address.
    lda #>(@returned-1)
    pha
    lda #<(@returned-1)
    pha
    jmp (entry)
@returned:
    lda entry
    ldx entry+1
    clc
    rts
@failed:
    ; Abort only an active temporary source save. Keep the original frontend
    ; error and backend status as the primary failure report.
    jsr cc_source_is_active
    beq :+
    jsr cc_source_abort
:
    sec
    rts

; Start one top-level runtime chunk. #294 uses the same operation after a
; named definition owner has completed and published.
fe_start_chunk:
    jsr cc_begin_owner
    lda cc_status
    beq @started
    jmp backend_error
@started:
    clc
    rts

; Complete the active top-level chunk and retain only its completed u16 entry.
; The fixed table supports 52 possible named owners plus the surrounding
; source chunks before, between, and after their definitions.
fe_finish_chunk:
    lda chunk_count
    cmp #53
    bcc @capacity_available
    ; Chunk targets are frontend-owned compile state, not backend status.
    lda #4
    sta fe_compile_status
    sec
    rts
@capacity_available:
    jsr cc_complete_owner
    sta chunk_target
    stx chunk_target+1
    lda cc_status
    beq @completed
    jmp backend_error
@completed:
    lda chunk_count
    asl a
    tay
    lda chunk_target
    sta top_level_chunk_targets,y
    iny
    lda chunk_target+1
    sta top_level_chunk_targets,y
    inc chunk_count
    clc
    rts

; Build a completed executable that calls every top-level chunk in source order.
; Named definition owners are deliberately absent from this runtime sequence.
fe_build_driver:
    jsr cc_begin_owner
    lda cc_status
    beq @driver_started
    jmp backend_error
@driver_started:
    lda #0
    sta chunk_index
@next_chunk:
    lda chunk_index
    cmp chunk_count
    beq @complete_driver
    asl a
    tay
    lda top_level_chunk_targets,y
    sta chunk_target
    iny
    lda top_level_chunk_targets,y
    tax
    lda chunk_target
    jsr cc_call
    lda cc_status
    beq @chunk_called
    jmp backend_error
@chunk_called:
    inc chunk_index
    jmp @next_chunk
@complete_driver:
    jsr cc_complete_owner
    sta entry
    stx entry+1
    lda cc_status
    beq @driver_completed
    jmp backend_error
@driver_completed:
    lda entry
    ldx entry+1
    clc
    rts

source_error:
    lda #1
    bne set_error
syntax_error:
    lda #2
    bne set_error
literal_error:
    lda #3
    bne set_error
backend_error:
    lda #4
set_error:
    sta fe_compile_status
    sec
    rts

; Fill the single lookahead byte. Frame end is cached and never read again.
peek:
    lda look_state
    bne @ready
    jsr fe_next
    bcc @byte
    lda fe_status
    beq @end
    lda #3
    bne @state
@end:
    lda #2
@state:
    sta look_state
    rts
@byte:
    sta look_byte
    lda #1
    sta look_state
@ready:
    rts

take:
    lda #0
    sta look_state
    rts

; Skip logical-line separators and comments, leaving the first target byte.
skip_separators:
    jsr peek
    lda look_state
    cmp #1
    jne @done
    lda look_byte
    cmp #';'
    beq @comment
    cmp #' '
    beq @skip
    cmp #9
    beq @skip
    cmp #10
    beq @skip
    cmp #13
    jne @done
@skip:
    jsr take
    jmp skip_separators
@comment:
    jsr take
@comment_byte:
    jsr peek
    lda look_state
    cmp #1
    bne @done
    lda look_byte
    cmp #10
    beq @skip
    jsr take
    jmp @comment_byte
@done:
    rts

statement:
    lda look_byte
    cmp #'='
    jeq fe_definition
    lda look_byte
    sta target_index
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'='
    jne syntax_error
    jsr take
    lda target_index
    cmp #'^'
    jeq fe_label
    cmp #'#'
    jeq fe_label
    cmp #'%'
    jeq @conditional
    cmp #'*'
    jeq @while
    cmp #'&'
    jeq @write_definition
    lda target_index
    cmp #'~'
    beq @target_ok
    cmp #'A'
    bcc @special
    cmp #'Z'+1
    bcc @target_ok
    cmp #'a'
    bcc @special
    cmp #'z'+1
    bcc @target_ok
@special:
    cmp #'@'
    beq @target_ok
    cmp #'?'
    beq @target_ok
    cmp #'$'
    jne syntax_error
@target_ok:
    lda target_index
    cmp #'a'
    jcc @compile_operand
    cmp #'z'+1
    jcs @compile_operand
    ; Resolve at compile time. Runtime code calls only the resolved target.
    lda #0
    ldx target_index
    jsr cc_resolve
    sta call_target
    stx call_target+1
    lda cc_status
    jne backend_error
    ; `p=()` has no operands; keep its parentheses out of value().
    jsr peek
    lda look_state
    cmp #1
    bne @compile_operand
    lda look_byte
    cmp #'('
    bne @compile_operand
    jsr take
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #')'
    bne @call_group
    jsr take
    jsr peek
    jsr statement_end
    lda fe_compile_status
    jne @done
    lda call_target
    ldx call_target+1
    jmp cc_call
@call_group:
    ; The opening parenthesis was consumed to distinguish `()` from a normal
    ; grouped operand. Compile the group contents, then continue the ordinary
    ; operator/comma path with its result already on the value stack.
    jsr enter_nesting
    lda fe_compile_status
    jne @done
    lda #0
    jsr operand
    lda fe_compile_status
    jne @done
    jsr close_group
    dec nesting
    lda fe_compile_status
    jne @done
    lda #1
    jsr operand
    lda fe_compile_status
    jne @done
    jmp @operands_ready
@compile_operand:
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'~'
    bne @value
    jsr take
    lda #1
    bne @operand
@value:
    lda #0
@operand:
    jsr operand
    lda fe_compile_status
    jne @done
@operands_ready:
@comma:
    jsr peek
    lda look_state
    cmp #1
    bne @end
    lda look_byte
    cmp #','
    bne @end
    jsr take
    lda #0
    jsr operand
    lda fe_compile_status
    beq @comma
    rts
@end:
    jsr statement_end
    lda fe_compile_status
    jne @done
    lda target_index
    cmp #'~'
    jeq @done
    cmp #'a'
    bcc @ordinary_store
    cmp #'z'+1
    bcc @write_call
@ordinary_store:
    cmp #'A'
    bcc @write_special
    cmp #'Z'+1
    bcs @write_special
    sec
    sbc #'A'
    tax
    jmp cc_store_reg
@write_special:
    cmp #'@'
    bne @number
    lda #<rt_store_storage
    ldx #>rt_store_storage
    jmp cc_call
@number:
    cmp #'?'
    bne @char
    lda #<rt_print_number
    ldx #>rt_print_number
    jmp cc_call
@char:
    lda #<rt_print_char
    ldx #>rt_print_char
    jmp cc_call
@write_call:
    lda call_target
    ldx call_target+1
    jmp cc_call
@conditional:
    lda structured_depth
    cmp #16
    jcs syntax_error
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'['
    jeq @condition_empty
    lda #0
    jsr operand
    lda fe_compile_status
    jne @conditional_done
    lda cc_status
    jne backend_error
    jsr statement_end
    lda fe_compile_status
    jne @conditional_done
    jsr cc_jz_placeholder
    jsr cc_work_push
    lda cc_status
    jne backend_error
    jsr required_block
    lda fe_compile_status
    bne @conditional_done
    jsr skip_separators
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    bne @one_arm
    lda look_byte
    cmp #'['
    bne @one_arm
    ; Keep this IF's work-stack depth constant while replacing false with end.
    jsr cc_work_pop
    sta patch_scratch
    stx patch_scratch+1
    lda cc_status
    jne backend_error
    jsr cc_jump_placeholder
    jsr cc_work_push
    lda cc_status
    jne backend_error
    lda patch_scratch
    ldx patch_scratch+1
    jsr cc_patch_here
    lda cc_status
    jne backend_error
    jsr required_block
    lda fe_compile_status
    bne @conditional_done
    jsr cc_work_pop
    jsr cc_patch_here
    lda cc_status
    jne backend_error
    rts
@one_arm:
    jsr cc_work_pop
    jsr cc_patch_here
    lda cc_status
    jne backend_error
    rts
@condition_empty:
    jmp syntax_error
@conditional_done:
    rts
@while:
    ; `*=()` is an exact header. Keep the expression multiplication operator
    ; untouched; only statement target `*=` enters this source form.
    lda while_depth
    cmp #16
    jcs syntax_error
    lda while_depth
    sta while_slot
    inc while_depth

    jsr peek
    lda look_state
    cmp #1
    jne @while_syntax
    lda look_byte
    cmp #'('
    jne @while_syntax
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne @while_syntax
    lda look_byte
    cmp #')'
    jne @while_syntax
    jsr take
    jsr require_form_boundary
    lda fe_compile_status
    jne @while_done

    ; Frame arrays are indexed independently of the IF work stack, preserving
    ; sixteen nested While forms without using two work-stack words per level.
    lda while_slot
    asl a
    sta while_offset
    jsr cc_mark
    sta while_word
    stx while_word+1
    ldy while_offset
    lda while_word
    sta while_loop_targets,y
    iny
    lda while_word+1
    sta while_loop_targets,y

    jsr required_block
    lda fe_compile_status
    bne @while_done
    lda while_depth
    sec
    sbc #1
    asl a
    sta while_offset
    jsr cc_jz_placeholder
    sta while_word
    stx while_word+1
    lda while_offset
    tay
    lda while_word
    sta while_exit_patches,y
    iny
    lda while_word+1
    sta while_exit_patches,y

    jsr required_block
    lda fe_compile_status
    bne @while_done
    lda while_depth
    sec
    sbc #1
    asl a
    sta while_offset
    ldy while_offset
    lda while_loop_targets,y
    sta while_word
    iny
    lda while_loop_targets,y
    tax
    lda while_word
    jsr cc_jump_to
    lda cc_status
    jne @while_backend
    ldy while_offset
    lda while_exit_patches,y
    sta while_word
    iny
    lda while_exit_patches,y
    tax
    lda while_word
    jsr cc_patch_here
    lda cc_status
    jne @while_backend
    dec while_depth
    rts
@while_syntax:
    jmp syntax_error
@while_backend:
    jmp backend_error
@while_done:
    rts
@done:
    rts

; Top-level `&=p [ ... ]` creates a fresh owner and publishes its completed
; Write target only after required-block compilation and owner completion.
@write_definition:
    lda structured_depth
    jne @write_definition_syntax
    jsr peek
    lda look_state
    cmp #1
    jne @write_definition_syntax
    lda look_byte
    cmp #'a'
    jcc @write_definition_syntax
    cmp #'z'+1
    jcs @write_definition_syntax
    sta definition_identity
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne @write_definition_syntax
    lda look_byte
    cmp #'{'
    jeq @template_definition
    jsr require_form_boundary
    lda fe_compile_status
    bne @write_definition_done
    lda definition_identity
    jsr template_is_bound
    bne @write_definition_syntax
    lda #0
    ldx definition_identity
    jsr cc_is_bound
    bcs @write_definition_syntax
    lda cc_status
    jne @write_definition_backend

    ; Preserve source order by finishing the prior chunk before the named
    ; owner, then resume with a fresh chunk after publication.
    jsr fe_finish_chunk
    lda fe_compile_status
    bne @write_definition_done
    jsr cc_begin_owner
    lda cc_status
    jne @write_definition_backend
    jsr required_block
    lda fe_compile_status
    bne @write_definition_done
    jsr cc_complete_owner
    sta cc_arg
    stx cc_arg+1
    lda cc_status
    jne @write_definition_backend
    lda #0
    ldx definition_identity
    jsr cc_publish
    lda cc_status
    jne @write_definition_backend
    jsr fe_start_chunk
    rts
@write_definition_syntax:
    jmp syntax_error
@write_definition_backend:
    jmp backend_error
@write_definition_done:
    rts

; Parse and save a source template without compiling any body RHS.
@template_definition:
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne @write_definition_syntax
    lda look_byte
    cmp #'}'
    jne @write_definition_syntax
    jsr take
    jsr require_form_boundary
    lda fe_compile_status
    bne @write_definition_done

    lda definition_identity
    jsr template_is_bound
    bne @write_definition_syntax
    lda #0
    ldx definition_identity
    jsr cc_is_bound
    bcs @write_definition_syntax
    lda cc_status
    jne @write_definition_backend
    lda #1
    ldx definition_identity
    jsr cc_is_bound
    bcs @write_definition_syntax
    lda cc_status
    jne @write_definition_backend

    jsr skip_separators
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    jne @write_definition_syntax
    lda look_byte
    cmp #'['
    jne @write_definition_syntax
    jsr take
    jsr require_form_boundary
    lda fe_compile_status
    bne @write_definition_done
    lda look_state
    cmp #2
    jeq @write_definition_syntax

    ; Only after every header check and both namespaces are known to be free.
    jsr fe_finish_chunk
    lda fe_compile_status
    bne @write_definition_done
    jsr cc_source_begin
    lda cc_status
    jne @write_definition_backend
    lda #1
    sta template_first
    lda #0
    sta template_seen_hole
    sta template_statement_count
@template_body_next:
    jsr template_skip_separators
    lda fe_compile_status
    jne @write_definition_done
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    jeq @write_definition_syntax
    lda look_byte
    cmp #']'
    jeq @template_body_close
    jsr template_body_statement
    lda fe_compile_status
    jne @write_definition_done
    lda cc_status
    jne @write_definition_backend
    jmp @template_body_next
@template_body_close:
    lda template_statement_count
    jeq @write_definition_syntax
    lda template_seen_hole
    jeq @write_definition_syntax
    jsr take
    jsr require_form_boundary
    lda fe_compile_status
    jne @write_definition_done
    jsr cc_source_finish
    sta template_saved_start
    stx template_saved_start+1
    lda cc_status
    jne @write_definition_backend
    lda template_saved_start
    sta cc_arg
    lda template_saved_start+1
    sta cc_arg+1
    lda definition_identity
    jsr template_publish
    lda cc_status
    jne @write_definition_backend
    jsr fe_start_chunk
    rts

; Skip source separators and comments inside the template body. Comments
; never enter the saved source and all physical newlines become canonical LF.
template_skip_separators:
    jsr peek
    lda look_state
    cmp #1
    jne @done
    lda look_byte
    cmp #';'
    beq @comment
    cmp #' '
    beq @skip
    cmp #9
    beq @skip
    cmp #10
    beq @skip
    cmp #13
    bne @done
@skip:
    jsr take
    jmp template_skip_separators
@comment:
    jsr take
@comment_next:
    jsr peek
    lda look_state
    cmp #1
    bne @done
    lda look_byte
    cmp #10
    beq @skip
    jsr take
    jmp @comment_next
@done:
    rts

; Validate the assignment prefix before saving anything from this statement.
template_body_statement:
    lda look_byte
    cmp #'A'
    bcc @hole_prefix
    cmp #'Z'+1
    bcc @register_prefix
@hole_prefix:
    cmp #'{'
    jne @bad
    ; Verify the complete {}= prefix before writing any of it.
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne @bad
    lda look_byte
    cmp #'}'
    jne @bad
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne @bad
    lda look_byte
    cmp #'='
    jne @bad
    jsr template_begin_statement
    lda fe_compile_status
    bne @done
    lda #'{'
    jsr template_append
    lda #'}'
    jsr template_append
    inc template_seen_hole
    jsr template_save_equals
    rts
@register_prefix:
    sta template_char
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne @bad
    lda look_byte
    cmp #'='
    jne @bad
    jsr template_begin_statement
    lda fe_compile_status
    bne @done
    lda template_char
    jsr template_append
    jsr template_save_equals
    rts
@bad:
    jmp syntax_error
@done:
    rts

template_begin_statement:
    lda template_first
    bne @first
    lda #10
    jsr cc_source_append
    lda cc_status
    bne @backend
@first:
    lda #0
    sta template_first
    inc template_statement_count
    clc
    rts
@backend:
    jmp backend_error

template_save_equals:
    jsr template_take_save
    lda fe_compile_status
    bne @done
@rhs:
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    beq @end
    lda look_byte
    cmp #' '
    beq @end
    cmp #9
    beq @end
    cmp #10
    beq @end
    cmp #13
    beq @end
    cmp #';'
    beq @end
    cmp #']'
    beq @bad
    cmp #'['
    beq @bad
    cmp #'}'
    beq @bad
    cmp #'{'
    bne @save_rhs
    jsr template_take_save
    lda fe_compile_status
    bne @done
    jsr peek
    lda look_state
    cmp #1
    jne @bad
    lda look_byte
    cmp #'}'
    jne @bad
    jsr template_take_save
    lda fe_compile_status
    bne @done
    inc template_seen_hole
    jmp @rhs
@save_rhs:
    jsr template_take_save
    lda fe_compile_status
    bne @done
    jmp @rhs
@end:
    rts
@bad:
    jmp syntax_error
@backend:
    jmp backend_error
@done:
    rts

; Consume current lookahead and append it verbatim to the arena. A backend
; capacity failure is promoted to frontend resource status 4 by the caller.
template_take_save:
    lda look_byte
    sta template_char
    jsr take
    lda template_char
    jmp template_append
@done:
    rts

template_append:
    jsr cc_source_append
    lda cc_status
    beq @done
    jmp backend_error
@done:
    rts

; Numeric label operands reuse the compile-run constant table, then delegate
; all owner-local definition/fixup behavior to the backend.
fe_label:
    jsr resolve_label_operand
    lda fe_compile_status
    jne @done
    jsr statement_end
    lda fe_compile_status
    jne @done
    lda target_index
    cmp #'^'
    bne @jump
    lda literal
    ldx literal+1
    jmp cc_define_label
@jump:
    lda literal
    ldx literal+1
    jmp cc_label_jump
@done:
    rts

; Top-level `==NAME,value`. The table is compile-run local and never emitted.
fe_definition:
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'='
    jne syntax_error
    jsr take
    jsr scan_constant_name
    lda fe_compile_status
    bne @definition_done
    jsr constant_find
    bcc :+
    jmp syntax_error             ; duplicate
:
    lda constant_count
    cmp #16
    bcc :+
    jmp syntax_error
:
    jsr expect_comma
    lda fe_compile_status
    bne @definition_done
    jsr peek
    lda look_state
    cmp #1
    jne literal_error
    jsr parse_literal
    lda fe_compile_status
    bne @definition_done
    jsr statement_end
    lda fe_compile_status
    bne @definition_done
    ; Validate everything before publishing a table entry.
    lda constant_count
    sta constant_slot
    jsr constant_address
    ldy #0
    ldx #0
@copy_name:
    cpx constant_length
    beq @store_value
    lda constant_name,x
    sta (constant_ptr),y
    iny
    inx
    bne @copy_name
@store_value:
    lda #0
    sta (constant_ptr),y          ; terminator (not needed for compare)
    iny
    lda literal
    sta (constant_ptr),y
    iny
    lda literal+1
    sta (constant_ptr),y
    inc constant_count
@definition_done:
    rts

expect_comma:
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #','
    jne syntax_error
    jmp take

; Resolve decimal or named integer label operand into literal, restricted to
; the non-negative signed-i16 range shared with the portable numeric labels.
resolve_label_operand:
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'-'
    beq @decimal
    cmp #'0'
    bcc @named
    cmp #'9'+1
    bcc @decimal
@named:
    jsr scan_constant_name
    lda fe_compile_status
    bne @done
    jsr constant_find
    bcs :+
    jmp syntax_error
:
    jsr constant_load_value
    jmp @range
@decimal:
    jsr parse_literal
    lda fe_compile_status
    bne @done
    lda negative
    bne @bad_range
@range:
    ; A set high bit is either a negative named value or >32767.
    lda literal+1
    bmi @bad_range
    clc
    rts
@bad_range:
    jmp literal_error
@done:
    rts

; Scan [A-Z][A-Z0-9_]+ into a 16-byte temporary with a bounded lookahead.
scan_constant_name:
    lda #0
    sta constant_length
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'A'
    jcc syntax_error
    cmp #'Z'+1
    jcs syntax_error
    lda #0
    sta constant_length
scan_constant_tail:
@name_byte:
    lda look_byte
    sta constant_char
    lda constant_length
    cmp #16
    bcc :+
    jmp syntax_error
:
    tax
    lda constant_char
    sta constant_name,x
    inc constant_length
    jsr take
    jsr peek
    lda look_state
    cmp #1
    bne @name_end
    lda look_byte
    cmp #'A'
    bcc @not_upper
    cmp #'Z'+1
    bcc @name_byte
@not_upper:
    cmp #'0'
    bcc @name_end
    cmp #'9'+1
    bcc @name_byte
    cmp #'_'
    beq @name_byte
@name_end:
    lda constant_length
    cmp #2
    bcs :+
    jmp syntax_error
:
    rts

; Carry set means the bounded name is already present.
constant_find:
    lda #0
    sta constant_slot
@find_slot:
    lda constant_slot
    cmp constant_count
    beq @not_found
    jsr constant_address
    ldy #0
    ldx #0
@compare:
    cpx constant_length
    beq @check_end
    lda (constant_ptr),y
    cmp constant_name,x
    bne @next_slot
    iny
    inx
    bne @compare
@check_end:
    lda (constant_ptr),y
    beq @found
@next_slot:
    inc constant_slot
    jmp @find_slot
@found:
    sec
    rts
@not_found:
    clc
    rts

constant_load_value:
    ; Move from name start to the saved two-byte value.
    jsr constant_address
    lda constant_length
    tay
    iny
    lda (constant_ptr),y
    sta literal
    iny
    lda (constant_ptr),y
    sta literal+1
    rts

; Convert constant_slot to a full 16-bit address. Entry offsets are 19 bytes;
; repeated pointer increments avoid truncating offsets above 255 into Y.
constant_address:
    lda #<constant_table
    sta constant_ptr
    lda #>constant_table
    sta constant_ptr+1
    lda constant_slot
    sta constant_index
@advance:
    lda constant_index
    beq @address_ready
    clc
    lda constant_ptr
    adc #19
    sta constant_ptr
    lda constant_ptr+1
    adc #0
    sta constant_ptr+1
    dec constant_index
    jmp @advance
@address_ready:
    rts

; Whitespace, newline, comment, or frame end terminates one statement.
statement_end:
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    beq @ok
    lda look_byte
    cmp #' '
    beq @ok
    cmp #9
    beq @ok
    cmp #10
    beq @ok
    cmp #13
    beq @ok
    cmp #';'
    jne syntax_error
@ok:
    rts

; Check the byte after a consumed delimiter without consuming a separator.
; Frame end is a valid boundary; opener callers reject it separately.
require_form_boundary:
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    beq @ok
    lda look_byte
    cmp #' '
    beq @ok
    cmp #9
    beq @ok
    cmp #10
    beq @ok
    cmp #13
    beq @ok
    cmp #';'
    jne syntax_error
@ok:
    rts

; Required source block, compiled inline into the current owner.
required_block:
    jsr skip_separators
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'['
    jne syntax_error
    lda structured_depth
    cmp #16
    jcs syntax_error
    inc structured_depth
    jsr take
    jsr require_form_boundary
    lda fe_compile_status
    bne @block_done
    lda look_state
    cmp #2
    jeq syntax_error
@block_next:
    jsr skip_separators
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    jeq syntax_error
    lda look_byte
    cmp #']'
    beq @block_close
    jsr statement
    lda fe_compile_status
    bne @block_done
    lda cc_status
    jne backend_error
    jmp @block_next
@block_close:
    jsr take
    jsr require_form_boundary
    lda fe_compile_status
    bne @block_done
    dec structured_depth
    rts
@block_done:
    rts

; A=1 means the existing stack top seeds this operand. Operators are emitted
; as encountered, so every binary operation is left associative.
operand:
    bne @operators
    jsr value
    lda fe_compile_status
    bne @done
@operators:
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #2
    beq @done
    lda look_byte
    cmp #','
    beq @done
    cmp #')'
    beq @done
    cmp #' '
    beq @done
    cmp #9
    beq @done
    cmp #10
    beq @done
    cmp #13
    beq @done
    cmp #';'
    beq @done
    jsr operator
    pha
    lda fe_compile_status
    beq @operator_ready
    pla
    rts
@operator_ready:
    txa
    pha
    jsr value
    pla
    tax
    pla
    ldy fe_compile_status
    bne @done
    jsr cc_call
    lda cc_status
    jne backend_error
    jmp @operators
@done:
    rts

value:
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'('
    jeq @group
    cmp #'0'
    bcc @minus
    cmp #'9'+1
    jcc @literal
@minus:
    cmp #'-'
    jeq @literal
    cmp #'A'
    bcc @applied
    cmp #'Z'+1
    bcs @applied
    sec
    sbc #'A'
    pha
    jsr take
    jsr peek
    lda look_state
    cmp #1
    bne @register
    lda look_byte
    cmp #'A'
    bcc @name_digit
    cmp #'Z'+1
    bcc @named
@name_digit:
    lda look_byte
    cmp #'0'
    bcc @register
    cmp #'9'+1
    bcc @named
    cmp #'_'
    bne @register
@named:
    pla
    clc
    adc #'A'
    sta constant_name
    lda #1
    sta constant_length
    jsr scan_constant_tail
    lda fe_compile_status
    beq :+
    jmp @done
:
    jsr constant_find
    bcs :+
    jmp syntax_error
:
    jsr constant_load_value
    lda literal
    ldx literal+1
    jmp cc_push_const
@register:
    pla
    tax
    jmp cc_load_reg
@applied:
    cmp #'@'
    jne syntax_error
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #'('
    jne syntax_error
    jsr take
    jsr enter_nesting
    lda fe_compile_status
    bne @done
    lda #0
    jsr operand
    lda fe_compile_status
    bne @leave
@applied_comma:
    jsr peek
    lda look_state
    cmp #1
    bne @close_applied
    lda look_byte
    cmp #','
    bne @close_applied
    jsr take
    lda #0
    jsr operand
    lda fe_compile_status
    beq @applied_comma
@close_applied:
    jsr close_group
@leave:
    dec nesting
    lda fe_compile_status
    bne @done
    lda #<rt_load_storage
    ldx #>rt_load_storage
    jmp cc_call
@literal:
    jsr parse_literal
    lda fe_compile_status
    bne @done
    lda literal
    ldx literal+1
    jmp cc_push_const
@group:
    jsr take
    jsr enter_nesting
    lda fe_compile_status
    bne @done
    lda #0
    jsr operand
    lda fe_compile_status
    bne @leave_group
    jsr close_group
@leave_group:
    dec nesting
@done:
    rts

enter_nesting:
    inc nesting
    lda nesting
    cmp #17
    bcc @ok
    jmp syntax_error
@ok:
    rts

close_group:
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    jne syntax_error
    lda look_byte
    cmp #')'
    jne syntax_error
    jmp take

; Signed decimal literal. Check before multiply so overflow cannot wrap.
parse_literal:
    lda #0
    sta negative
    sta literal
    sta literal+1
    lda look_byte
    cmp #'-'
    bne @digits
    lda #1
    sta negative
    jsr take
    jsr peek
    lda look_state
    cmp #1
    jne literal_error
@digits:
    lda look_byte
    cmp #'0'
    jcc literal_error
    cmp #'9'+1
    jcs literal_error
@loop:
    lda look_byte
    sec
    sbc #'0'
    sta digit
    lda literal+1
    cmp #>3276
    bcc @multiply
    jne literal_error
    lda literal
    cmp #<3276
    bcc @multiply
    jne literal_error
    lda digit
    cmp #8
    bcc @multiply
    jne literal_error
    lda negative
    jeq literal_error
@multiply:
    ; 10*x = 2*x + 8*x, using a temporary on the CPU stack.
    asl literal
    rol literal+1
    lda literal+1
    pha
    lda literal
    pha
    asl literal
    rol literal+1
    asl literal
    rol literal+1
    pla
    clc
    adc literal
    sta literal
    pla
    adc literal+1
    sta literal+1
    lda literal
    clc
    adc digit
    sta literal
    bcc @next
    inc literal+1
@next:
    jsr take
    jsr peek
    lda look_state
    cmp #3
    jeq source_error
    cmp #1
    bne @finish
    lda look_byte
    cmp #'0'
    bcc @finish
    cmp #'9'+1
    bcc @loop
@finish:
    lda negative
    beq @emit
    sec
    lda #0
    sbc literal
    sta literal
    lda #0
    sbc literal+1
    sta literal+1
@emit:
    rts

; Built-in binary role resolution. Composite comparison consumes two bytes.
operator:
    lda look_byte
    cmp #'='
    beq @equal
    cmp #'!'
    beq @not_equal
    cmp #'<'
    beq @less
    cmp #'>'
    beq @greater
    ldx #0
@find:
    cmp operator_chars,x
    beq @found
    inx
    cpx #7
    bne @find
    jmp syntax_error
@found:
    stx operator_index
    jsr take
    ldy operator_index
    lda operator_low,y
    ldx operator_high,y
    rts
@equal:
    lda #<rt_eq
    ldx #>rt_eq
    jmp @double
@not_equal:
    lda #<rt_ne
    ldx #>rt_ne
@double:
    pha
    txa
    pha
    jsr take
    jsr peek
    lda look_state
    cmp #1
    bne @bad_double
    lda look_byte
    cmp #'='
    bne @bad_double
    jsr take
    pla
    tax
    pla
    rts
@bad_double:
    pla
    pla
    jmp syntax_error
@less:
    lda #0
    sta operator_index
    lda #<rt_lt
    ldx #>rt_lt
    jmp @optional_equal
@greater:
    lda #1
    sta operator_index
    lda #<rt_gt
    ldx #>rt_gt
@optional_equal:
    pha
    txa
    pha
    jsr take
    jsr peek
    lda look_state
    cmp #1
    bne @single
    lda look_byte
    cmp #'='
    bne @single
    jsr take
    pla
    pla
    lda operator_index
    bne @greater_equal
    lda #<rt_le
    ldx #>rt_le
    rts
@greater_equal:
    lda #<rt_ge
    ldx #>rt_ge
    rts
@single:
    pla
    tax
    pla
    rts
