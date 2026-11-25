; Exercise the private arithmetic ABI without initializing the runtime.
.setcpu "6502"
.export _main
.import cell_add, cell_sub, cell_mul, serial_out, halt
.import rt_rng_state
.importzp zp_phase, rt_lhs, rt_rhs, rt_result

.macro CALC entry, lhs, rhs, expected
    lda #<(lhs & $ffff)
    sta rt_lhs
    lda #>(lhs & $ffff)
    sta rt_lhs+1
    lda #<(rhs & $ffff)
    sta rt_rhs
    lda #>(rhs & $ffff)
    sta rt_rhs+1
    jsr entry
    lda rt_result
    cmp #<(expected & $ffff)
    beq :+
    jmp bad
:
    lda rt_result+1
    cmp #>(expected & $ffff)
    beq :+
    jmp bad
:
.endmacro

.segment "CODE"
_main:
    cld
    ldx #99
    lda #$a5
    sta rt_rng_state
    sta rt_rng_state+1
@fill:
    sta zp_phase,x
    dex
    bpl @fill
    ; Mixed calls deliberately leave the previous arithmetic scratch intact.
    CALC cell_add, 32767, 1, -32768
    CALC cell_sub, -32768, 1, 32767
    CALC cell_mul, -32768, -1, -32768
    CALC cell_add, -7, 3, -4
    CALC cell_mul, -7, 3, -21
    CALC cell_sub, 0, -1, 1
    CALC cell_mul, 0, -32768, 0
    CALC cell_add, 0, 0, 0
    ldx #84
@check_compile:
    lda zp_phase,x
    cmp #$a5
    bne bad
    dex
    bpl @check_compile
    ldx #99
@check_runtime_tail:
    lda zp_phase,x
    cmp #$a5
    bne bad
    dex
    cpx #90
    bne @check_runtime_tail
    lda rt_rng_state
    cmp #$a5
    bne bad
    lda rt_rng_state+1
    cmp #$a5
    bne bad
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
bad:
    lda #'F'
    jsr serial_out
    lda #1
    jmp halt
