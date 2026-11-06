.setcpu "6502"
.export _main
.import fe_compile_run, halt
.importzp fe_compile_status

.segment "CODE"
_main:
    jsr fe_compile_run
    bcs @failed
    lda #0
    jmp halt
@failed:
    lda fe_compile_status
    jmp halt
