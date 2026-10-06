.setcpu "6502"
.export _main
.import fe_compile_run

.segment "CODE"
_main:
    jmp fe_compile_run
