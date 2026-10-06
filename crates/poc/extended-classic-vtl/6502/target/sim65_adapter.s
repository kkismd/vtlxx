; sim6502-specific bridge. A carries one byte for SERIAL_IN / SERIAL_OUT,
; and an 8-bit process status for HALT. This is not the ECVTL runtime ABI.
.setcpu "6502"

.export serial_in, serial_out, halt
.import _getchar, _putchar, exit

.segment "CODE"

serial_in:
    jsr _getchar                 ; cc65 int result is in A/X; smoke uses one byte
    cpx #$ff                     ; getchar returns -1 (EOF) as $ffff
    beq @eof
    clc                          ; carry clear: A contains a byte, including $ff
    rts
@eof:
    sec                          ; carry set: transport ended, A is not a byte
    rts

serial_out:
    ldx #$00                    ; cc65 putchar(int) takes A/X
    jmp _putchar

halt:
    jmp exit                    ; sim6502.lib passes A to simulator status
