; Exercise every bit position independently, including both truth-table inputs.
.setcpu "6502"
.export _main
.import rt_init, rt_push, rt_and, rt_or, rt_xor, serial_out, halt
.importzp rt_depth, rt_stack
.include "runtime_macros.inc"

.macro CHECK op, lhs, rhs, result
    jsr rt_init
    PUSH $1234
    PUSH lhs
    PUSH rhs
    ; The result must not depend on the incoming decimal or carry flags.
    sed
.if (rhs) & 1
    sec
.else
    clc
.endif
    jsr op
    cld
    lda #<(result)
    ldx #>(result)
    jsr check_result
.endmacro

.segment "CODE"
_main:
.repeat 16, bit
    CHECK rt_and, 0, 0, 0
    CHECK rt_and, (1 << bit), 0, 0
    CHECK rt_and, 0, (1 << bit), 0
    CHECK rt_and, (1 << bit), (1 << bit), (1 << bit)
    CHECK rt_or, 0, 0, 0
    CHECK rt_or, (1 << bit), 0, (1 << bit)
    CHECK rt_or, 0, (1 << bit), (1 << bit)
    CHECK rt_or, (1 << bit), (1 << bit), (1 << bit)
    CHECK rt_xor, 0, 0, 0
    CHECK rt_xor, (1 << bit), 0, (1 << bit)
    CHECK rt_xor, 0, (1 << bit), (1 << bit)
    CHECK rt_xor, (1 << bit), (1 << bit), 0
.endrepeat
    CHECK rt_and, 255, -256, 0
    CHECK rt_or, 255, -256, -1
    CHECK rt_xor, 255, -256, -1
    CHECK rt_xor, -32768, 32767, -1
    CHECK rt_and, $a55a, $0ff0, $0550
    CHECK rt_or, $a55a, $0ff0, $affa
    CHECK rt_xor, $a55a, $0ff0, $aaaa
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt

check_result:
    cmp rt_stack+2
    bne bad
    cpx rt_stack+3
    bne bad
    lda rt_depth
    cmp #2
    bne bad
    lda rt_stack
    cmp #$34
    bne bad
    lda rt_stack+1
    cmp #$12
    bne bad
    rts
bad:
    cld
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt
