; Private bridge for the VTLXX mos6502 host. Link instead of sim65_adapter.s.
.setcpu "6502"

.export serial_in, serial_try_in, serial_out, halt, entropy16

.segment "CODE"

; The host returns Byte in A with X=0, EOF with X=2, and Fatal with X=3.
serial_in:
    jsr $fff0
    cpx #0
    beq @byte
    cpx #2
    beq @eof
    lda #6                       ; Fatal, including an invalid Wait status
    jmp halt
@byte:
    clc
    rts
@eof:
    sec
    rts

; The host returns X=0/1/2/3 for Byte/NotReady/EOF/Fatal respectively.
serial_try_in:
    jmp $ffef

serial_out:
    jmp $fff1

halt:
    jmp $fff2

entropy16:
.ifdef SIM65_ENTROPY_SEED
    lda #<SIM65_ENTROPY_SEED
    ldx #>SIM65_ENTROPY_SEED
    clc
.else
    sec
.endif
    rts
