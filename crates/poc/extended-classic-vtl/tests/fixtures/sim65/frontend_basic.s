.setcpu "6502"
.export _main
.import fe_compile_run, serial_in, serial_out, halt
.importzp fe_compile_status

.segment "CODE"
_main:
    jsr fe_compile_run
    bcs @failed
    ; The next byte still belongs to runtime input, after generated code ran.
    jsr serial_in
    bcs @failed
    jsr serial_out
    lda #0
    jmp halt
@failed:
    lda fe_compile_status
    jmp halt
