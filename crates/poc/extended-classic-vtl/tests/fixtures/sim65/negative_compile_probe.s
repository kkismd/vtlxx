; Test-only entry point for observing the backend cause after frontend cleanup.
; The compile/run path is identical to sim65_cli.s; only failure halt status differs.
.setcpu "6502"

.import fe_compile_run
.importzp fe_compile_status, cc_status
.import halt

.segment "CODE"
.export _main

_main:
    jsr fe_compile_run
    bcc @success
    lda fe_compile_status
    cmp #4
    bne @frontend_failure
    lda cc_status
    jmp halt
@frontend_failure:
    lda fe_compile_status
    jmp halt
@success:
    lda #0
    jmp halt
