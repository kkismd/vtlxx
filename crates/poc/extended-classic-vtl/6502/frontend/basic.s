; Streaming basic ECVTL source compiler and top-level compile/run driver.
; The only source state is one lookahead byte; no source text is retained.
.setcpu "6502"
.macpack longbranch

.export fe_compile_run
.exportzp fe_compile_status
.import fe_init, fe_next, cc_init, cc_begin_owner, cc_complete_owner
.import cc_push_const, cc_call, cc_emit_byte
.import rt_init, rt_load_reg, rt_store_reg, rt_load_storage, rt_store_storage
.import rt_add, rt_sub, rt_mul, rt_div, rt_rem
.import rt_eq, rt_ne, rt_lt, rt_le, rt_gt, rt_ge
.import rt_print_number, rt_print_char
.importzp fe_status, cc_status

.segment "ZEROPAGE"
fe_compile_status: .res 1       ; 0 success, 1 source/transport, 2 syntax, 3 literal, 4 backend
look_state:        .res 1       ; 0 empty, 1 byte, 2 frame end, 3 transport error
look_byte:         .res 1
target_index:      .res 1
nesting:           .res 1
negative:          .res 1
literal:           .res 2
digit:             .res 1
operator_index:    .res 1
entry:             .res 2

.segment "RODATA"
operator_chars: .byte '+','-','*','/','%','<','>'
operator_low:   .byte <rt_add,<rt_sub,<rt_mul,<rt_div,<rt_rem,<rt_lt,<rt_gt
operator_high:  .byte >rt_add,>rt_sub,>rt_mul,>rt_div,>rt_rem,>rt_lt,>rt_gt

.segment "CODE"
; Carry clear on success; A/X is the completed entry. Compile errors never
; enter generated code. The caller may inspect fe_compile_status/cc_status.
fe_compile_run:
    lda #0
    sta fe_compile_status
    sta look_state
    sta nesting
    jsr fe_init
    jcs source_error
    jsr cc_init
    jsr cc_begin_owner
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
    jsr cc_complete_owner
    pha
    lda cc_status
    beq @entry_ready
    pla
    jmp backend_error
@entry_ready:
    pla
    sta entry
    stx entry+1
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
    sec
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
    bne @done
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
    cmp #'~'
    beq @target_ok
    cmp #'A'
    bcc @special
    cmp #'Z'+1
    bcc @target_ok
@special:
    cmp #'@'
    beq @target_ok
    cmp #'?'
    beq @target_ok
    cmp #'$'
    jne syntax_error
@target_ok:
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
    bne @done
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
    bne @done
    lda target_index
    cmp #'~'
    beq @done
    cmp #'A'
    bcc @write_special
    cmp #'Z'+1
    bcs @write_special
    sec
    sbc #'A'
    pha
    lda #$a2                 ; LDX #register index
    jsr cc_emit_byte
    pla
    jsr cc_emit_byte
    lda #<rt_store_reg
    ldx #>rt_store_reg
    jmp cc_call
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
@done:
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
    jcc parse_literal
@minus:
    cmp #'-'
    jeq parse_literal
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
    cmp #'('
    beq @invalid_register_apply
@register:
    pla
    pha
    lda #$a2
    jsr cc_emit_byte
    pla
    jsr cc_emit_byte
    lda #<rt_load_reg
    ldx #>rt_load_reg
    jmp cc_call
@invalid_register_apply:
    pla
    jmp syntax_error
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
    lda literal
    ldx literal+1
    jmp cc_push_const

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
