.macro PUSH value
    lda #<(value)
    ldx #>(value)
    jsr rt_push
.endmacro

.macro NUMBER value
    PUSH value
    jsr rt_print_number
.endmacro

.macro SEP
    lda #','
    jsr serial_out
.endmacro

.macro DONE
    lda #0
    jmp halt
.endmacro
