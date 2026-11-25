; Test-only entry point for observing the backend cause after frontend cleanup.
; The compile/run path is identical to sim65_cli.s; only failure halt status differs.
.setcpu "6502"

.import fe_compile_run
.importzp fe_compile_status, cc_status, sp
.import halt

.segment "BSS"
saved_s:  .res 1
saved_sp: .res 2

.segment "CODE"
.export _main

_main:
    lda sp
    sta saved_sp
    lda sp+1
    sta saved_sp+1
    lda #$a5
    pha
    tsx
    stx saved_s
    jsr fe_compile_run
    bcc @success
    tsx
    cpx saved_s
    bne @bad_stack
    pla
    cmp #$a5
    bne @bad_stack
    lda sp
    cmp saved_sp
    bne @bad_stack
    lda sp+1
    cmp saved_sp+1
    bne @bad_stack
    lda fe_compile_status
    cmp #4
    bne @frontend_failure
    lda cc_status
    clc
    adc #32
    jmp halt
@frontend_failure:
    lda fe_compile_status
    clc
    adc #64
    jmp halt
@success:
    lda #0
    jmp halt
@bad_stack:
    lda #99
    jmp halt
