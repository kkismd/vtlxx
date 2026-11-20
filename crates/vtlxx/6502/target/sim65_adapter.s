; sim6502-specific bridge. A carries one byte for SERIAL_IN / SERIAL_OUT,
; and an 8-bit process status for HALT. This is not the ECVTL runtime ABI.
.setcpu "6502"

.export serial_in, serial_try_in, serial_out, halt, entropy16
.ifdef ENTROPY16_TEST
.export entropy_test_calls
.segment "DATA"
entropy_test_calls: .byte 0
.endif
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

; The standard sim65 stdin adapter cannot observe readiness. Fail closed
; without touching stdin; callers may distinguish unsupported input from EOF.
serial_try_in:
    ldx #3                       ; Fatal / unsupported
    rts

serial_out:
    ldx #$00                    ; cc65 putchar(int) takes A/X
    jmp _putchar

halt:
    jmp exit                    ; sim6502.lib passes A to simulator status

; The host selects either an available private raw16 constant or an
; unavailable provider. PRNG state and stepping remain in common runtime.
entropy16:
.ifdef ENTROPY16_TEST
    inc entropy_test_calls
.endif
.ifdef SIM65_ENTROPY_SEED
    lda #<SIM65_ENTROPY_SEED
    ldx #>SIM65_ENTROPY_SEED
    clc
.else
.ifdef SIM65_ENTROPY_UNAVAILABLE
    sec
.else
.ifdef ENTROPY16_TEST_SEQUENCE
    lda entropy_test_calls
    ldx #0
    clc
.else
.ifdef ENTROPY16_TEST_VALUE
    lda #<ENTROPY16_TEST_VALUE
    ldx #>ENTROPY16_TEST_VALUE
    clc
.else
    sec
.endif
.endif
.endif
.endif
    rts
