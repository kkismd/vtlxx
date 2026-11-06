.setcpu "6502"
.export _main
.import halt

.segment "CODE"
_main:
    lda #$07
    jmp halt
