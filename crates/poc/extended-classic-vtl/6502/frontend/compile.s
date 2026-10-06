; Streaming ECVTL compile-run driver. The only source-sized state is one
; bounded statement buffer; source bytes themselves are consumed once through
; fe_next and are never retained as a program image.
.setcpu "6502"

.export fe_compile_run
.import fe_init, fe_next
.import cc_init, cc_begin_owner, cc_complete_owner, cc_push_const, cc_call
.import cc_publish, cc_resolve
.import rt_init, rt_print_number, rt_print_char, rt_load_reg, rt_store_reg
.import rt_load_storage, rt_store_storage, rt_add, rt_sub, rt_mul, rt_div, rt_rem
.import rt_eq, rt_ne, rt_lt, rt_le, rt_gt, rt_ge
.import halt
.importzp fe_status, cc_status

.segment "BSS"
fe_token: .res 64
fe_length: .res 1
fe_pos: .res 1
fe_byte: .res 1
fe_target: .res 1
fe_term: .res 3
fe_negative: .res 1
fe_value: .res 2
fe_entry: .res 2
fe_helper: .res 2
fe_owner: .res 1

.segment "CODE"
; Consume [length][source], compile supported current scalar Write forms,
; then execute the completed top-level owner. Runtime input stays unread.
fe_compile_run:
    jsr rt_init
    jsr cc_init
    jsr fe_init
    bcc :+
    jmp fe_fail
:
    jsr cc_begin_owner
    sta fe_entry
    stx fe_entry+1
@next:
    jsr fe_read_statement
    bcs @frame_end
    jsr fe_compile_statement
    lda cc_status
    beq :+
    jmp fe_fail
:
    jmp @next
@frame_end:
    lda fe_status
    bne fe_fail
    lda fe_entry
    ldx fe_entry+1
    jsr cc_complete_owner
    lda cc_status
    bne fe_fail
    lda fe_entry
    ldx fe_entry+1
    jsr call_target
    lda #0
    jmp halt

fe_fail:
    lda #1
    jmp halt

; Return one whitespace/comment-delimited statement in fe_token, carry set at
; clean frame end. The fixed token cap is independent of total source length.
fe_read_statement:
@skip:
    jsr fe_next
    bcs @end
    cmp #';'
    beq @comment
    cmp #10
    beq @skip
    cmp #13
    beq @skip
    cmp #' '
    beq @skip
    cmp #9
    beq @skip
    sta fe_token
    lda #1
    sta fe_length
@body:
    jsr fe_next
    bcs @terminated
    cmp #';'
    beq @comment_after
    cmp #10
    beq @terminated
    cmp #13
    beq @terminated
    cmp #' '
    beq @terminated
    cmp #9
    beq @terminated
    ldx fe_length
    cpx #63
    bcc :+
    jmp fe_fail
:
    sta fe_token,x
    inc fe_length
    jmp @body
@comment_after:
    jsr @skip_comment
@terminated:
    ldx fe_length
    lda #0
    sta fe_token,x
    clc
    rts
@comment:
    jsr @skip_comment
    jmp @skip
@end:
    sec
    rts
@skip_comment:
    jsr fe_next
    bcs @comment_done
    cmp #10
    bne @skip_comment
@comment_done:
    rts

fe_compile_statement:
    lda fe_token
    sta fe_target
    lda fe_token+1
    cmp #'='
    beq :+
    jmp fe_fail
:
    lda #2
    sta fe_pos
    jsr fe_compile_expression
    bcc :+
    jmp fe_fail
:
    lda fe_target
    cmp #'~'
    beq @stack_output
    cmp #'?'
    beq @print_number
    cmp #'$'
    beq @print_char
    cmp #'A'
    bcc @storage
    cmp #'Z'+1
    bcs @storage
    sec
    sbc #'A'
    tax
    jmp fe_emit_register_store
@storage:
    lda fe_target
    cmp #'@'
    beq :+
    jmp fe_fail
:
    lda #<rt_store_storage
    ldx #>rt_store_storage
    jmp fe_emit_call
@print_number:
    lda #<rt_print_number
    ldx #>rt_print_number
    jmp fe_emit_call
@print_char:
    lda #<rt_print_char
    ldx #>rt_print_char
    jmp fe_emit_call
@stack_output:
    rts

fe_compile_expression:
    jsr fe_value_term
    bcc :+
    jmp @bad
:
@operator:
    ldy fe_pos
    cpy fe_length
    beq @good
    lda fe_token,y
    sta fe_byte
    iny
    sty fe_pos
    jsr fe_value_term
    bcc :+
    jmp @bad
:
    lda fe_byte
    jsr fe_operator_target
    bcc :+
    jmp @bad
:
    lda fe_helper
    ldx fe_helper+1
    jsr cc_call
    lda cc_status
    bne @bad
    jmp @operator
@good:
    clc
    rts
@bad:
    sec
    rts

; Decimal i16 and A-Z primary reads. Expressions use the reference's
; left-to-right operator semantics; all operators have equal precedence.
fe_value_term:
    ldy fe_pos
    cpy fe_length
    bcc :+
    jmp @bad
:
    lda fe_token,y
    cmp #'A'
    bcc @number
    cmp #'Z'+1
    bcs @number
    sec
    sbc #'A'
    tax
    lda #<rt_load_reg
    sta fe_helper
    lda #>rt_load_reg
    sta fe_helper+1
    ; register index is passed in X, so emit the fixed helper call template
    ; through a tiny target-specific trampoline in the generated owner.
    jsr fe_emit_register_read
    inc fe_pos
    clc
    rts
@number:
    lda #0
    sta fe_value
    sta fe_value+1
@digits:
    ldy fe_pos
    cpy fe_length
    bcs @emit_number
    lda fe_token,y
    cmp #'0'
    bcc @emit_number
    cmp #'9'+1
    bcs @emit_number
    sec
    sbc #'0'
    sta fe_term
    ; value = value * 10 + digit, modulo 16 bits; sign/range checked below.
    ; Preserve x, form x*8 + x*2.
    lda fe_value
    sta fe_term+1
    lda fe_value+1
    sta fe_term+2
    asl fe_value
    rol fe_value+1
    asl fe_value
    rol fe_value+1
    asl fe_value
    rol fe_value+1
    asl fe_term+1
    rol fe_term+2
    clc
    lda fe_value
    adc fe_term+1
    sta fe_value
    lda fe_value+1
    adc fe_term+2
    sta fe_value+1
    clc
    lda fe_value
    adc fe_term
    sta fe_value
    bcc :+
    inc fe_value+1
:
    inc fe_pos
    jmp @digits
@emit_number:
    lda fe_value
    ldx fe_value+1
    jsr cc_push_const
    lda cc_status
    bne @bad
    clc
    rts
@bad:
    sec
    rts

fe_operator_target:
    cmp #'+'
    beq @add
    cmp #'-'
    beq @sub
    cmp #'*'
    beq @mul
    cmp #'/'
    beq @div
    cmp #'%'
    beq @rem
    cmp #'<'
    beq @lt
    cmp #'>'
    beq @gt
    cmp #'='
    beq @eq
    cmp #'!'
    beq @ne
    sec
    rts
@add: lda #<rt_add
    ldx #>rt_add
    bne @save
@sub: lda #<rt_sub
    ldx #>rt_sub
    bne @save
@mul: lda #<rt_mul
    ldx #>rt_mul
    bne @save
@div: lda #<rt_div
    ldx #>rt_div
    bne @save
@rem: lda #<rt_rem
    ldx #>rt_rem
    bne @save
@lt: lda #<rt_lt
    ldx #>rt_lt
    bne @save
@gt: lda #<rt_gt
    ldx #>rt_gt
    bne @save
@eq: lda #<rt_eq
    ldx #>rt_eq
    bne @save
@ne: lda #<rt_ne
    ldx #>rt_ne
@save:
    sta fe_helper
    stx fe_helper+1
    clc
    rts

fe_emit_call:
    jsr cc_call
    lda cc_status
    beq @ok
    sec
    rts
@ok:
    clc
    rts

fe_emit_register_read:
    ; Emit LDX #index / JSR rt_load_reg as raw backend bytes.
    ; The backend intentionally exposes no opcode API, so use an addressable
    ; helper thunk table for the 26 fixed register identities.
    txa
    asl a
    tax
    lda fe_register_thunks,x
    pha
    lda fe_register_thunks+1,x
    tax
    pla
    jsr cc_call
    rts

fe_emit_register_store:
    txa
    asl a
    tax
    lda fe_store_thunks,x
    pha
    lda fe_store_thunks+1,x
    tax
    pla
    jmp fe_emit_call

fe_register_thunks:
    .word fe_load_A,fe_load_B,fe_load_C,fe_load_D,fe_load_E,fe_load_F,fe_load_G
    .word fe_load_H,fe_load_I,fe_load_J,fe_load_K,fe_load_L,fe_load_M,fe_load_N
    .word fe_load_O,fe_load_P,fe_load_Q,fe_load_R,fe_load_S,fe_load_T,fe_load_U
    .word fe_load_V,fe_load_W,fe_load_X,fe_load_Y,fe_load_Z
fe_store_thunks:
    .word fe_store_A,fe_store_B,fe_store_C,fe_store_D,fe_store_E,fe_store_F,fe_store_G
    .word fe_store_H,fe_store_I,fe_store_J,fe_store_K,fe_store_L,fe_store_M,fe_store_N
    .word fe_store_O,fe_store_P,fe_store_Q,fe_store_R,fe_store_S,fe_store_T,fe_store_U
    .word fe_store_V,fe_store_W,fe_store_X,fe_store_Y,fe_store_Z

.macro LOAD_THUNK name, index
name:
    ldx #index
    jmp rt_load_reg
.endmacro
LOAD_THUNK fe_load_A,0
LOAD_THUNK fe_load_B,1
LOAD_THUNK fe_load_C,2
LOAD_THUNK fe_load_D,3
LOAD_THUNK fe_load_E,4
LOAD_THUNK fe_load_F,5
LOAD_THUNK fe_load_G,6
LOAD_THUNK fe_load_H,7
LOAD_THUNK fe_load_I,8
LOAD_THUNK fe_load_J,9
LOAD_THUNK fe_load_K,10
LOAD_THUNK fe_load_L,11
LOAD_THUNK fe_load_M,12
LOAD_THUNK fe_load_N,13
LOAD_THUNK fe_load_O,14
LOAD_THUNK fe_load_P,15
LOAD_THUNK fe_load_Q,16
LOAD_THUNK fe_load_R,17
LOAD_THUNK fe_load_S,18
LOAD_THUNK fe_load_T,19
LOAD_THUNK fe_load_U,20
LOAD_THUNK fe_load_V,21
LOAD_THUNK fe_load_W,22
LOAD_THUNK fe_load_X,23
LOAD_THUNK fe_load_Y,24
LOAD_THUNK fe_load_Z,25

.macro STORE_THUNK name, index
name:
    ldx #index
    jmp rt_store_reg
.endmacro
STORE_THUNK fe_store_A,0
STORE_THUNK fe_store_B,1
STORE_THUNK fe_store_C,2
STORE_THUNK fe_store_D,3
STORE_THUNK fe_store_E,4
STORE_THUNK fe_store_F,5
STORE_THUNK fe_store_G,6
STORE_THUNK fe_store_H,7
STORE_THUNK fe_store_I,8
STORE_THUNK fe_store_J,9
STORE_THUNK fe_store_K,10
STORE_THUNK fe_store_L,11
STORE_THUNK fe_store_M,12
STORE_THUNK fe_store_N,13
STORE_THUNK fe_store_O,14
STORE_THUNK fe_store_P,15
STORE_THUNK fe_store_Q,16
STORE_THUNK fe_store_R,17
STORE_THUNK fe_store_S,18
STORE_THUNK fe_store_T,19
STORE_THUNK fe_store_U,20
STORE_THUNK fe_store_V,21
STORE_THUNK fe_store_W,22
STORE_THUNK fe_store_X,23
STORE_THUNK fe_store_Y,24
STORE_THUNK fe_store_Z,25

call_target:
    jmp (fe_entry)
