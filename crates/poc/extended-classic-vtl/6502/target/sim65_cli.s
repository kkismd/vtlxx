; Host CLI entry point. The compiler owns all source semantics and execution.
.setcpu "6502"

.import fe_compile_run
.importzp fe_compile_status
.import halt

.segment "CODE"
.export _main

_main:
    jsr fe_compile_run
    bcc @success
    ; Keep target failure distinct from sim65/tool failures and runtime HALT
    ; statuses. The low bits retain the frontend's existing status value.
    lda fe_compile_status
    ora #$80
    jmp halt
@success:
    lda #0
    jmp halt
