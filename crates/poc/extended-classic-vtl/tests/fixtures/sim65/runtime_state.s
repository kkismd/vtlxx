.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_load_reg, rt_store_reg
.import rt_load_storage, rt_store_storage, rt_print_number
.importzp rt_depth
.import serial_out, halt
.include "runtime_macros.inc"

.segment "CODE"
_main:
    jsr rt_init

    ldx #0
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    ldx #25
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    PUSH 0
    jsr rt_load_storage
    jsr rt_print_number
    SEP
    PUSH 127
    jsr rt_load_storage
    jsr rt_print_number
    SEP

    PUSH 4660
    ldx #0
    jsr rt_store_reg
    ldx #0
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    PUSH 22136
    ldx #25
    jsr rt_store_reg
    ldx #25
    jsr rt_load_reg
    jsr rt_print_number
    SEP

    PUSH 127
    PUSH 4660
    jsr rt_store_storage
    PUSH -1
    jsr rt_load_storage
    jsr rt_print_number
    SEP
    PUSH 128
    PUSH 22136
    jsr rt_store_storage
    PUSH 0
    jsr rt_load_storage
    jsr rt_print_number
    SEP
    PUSH -32768
    jsr rt_load_storage
    jsr rt_print_number
    SEP
    ldx #0
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    ldx #25
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    lda rt_depth
    beq @before_reinit_empty
    lda #9
    jmp halt
@before_reinit_empty:

    ; One initializer must clear every runtime state owner for a new run.
    jsr rt_init
    ldx #0
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    ldx #25
    jsr rt_load_reg
    jsr rt_print_number
    SEP
    PUSH 0
    jsr rt_load_storage
    jsr rt_print_number
    SEP
    PUSH 127
    jsr rt_load_storage
    jsr rt_print_number
    lda rt_depth
    beq @empty
    lda #9
    jmp halt
@empty:
    DONE
