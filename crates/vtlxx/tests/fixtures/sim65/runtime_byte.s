.setcpu "6502"
.macpack longbranch
.export _main, rt_test_probe
.import rt_init, rt_push, rt_peek, rt_poke, serial_out, halt
.importzp rt_depth, rt_stack
.include "runtime_macros.inc"

.segment "BSS"
guard_before: .res 1
target: .res 1
.assert target >= $100, lderror, "probe must exercise a nonzero address high byte"
guard_after: .res 1
index: .res 1

.segment "RODATA"
values_lo: .byte 0, 128, 255, 0, 255, 255
values_hi: .byte 0, 0, 0, 1, 255, 1

.segment "CODE"
_main:
    jsr rt_init
    lda #$a5
    sta guard_before
    sta target
    sta guard_after
.if TEST_CASE = 1
    PUSH $1234
    lda #0
    sta index
@loop:
    PUSH target
    ldx index
    lda values_lo,x
    pha
    lda values_hi,x
    tax
    pla
    jsr rt_push
    jsr rt_poke
    lda rt_depth
    cmp #1
    jne bad
    ldx index
    lda target
    cmp values_lo,x
    jne bad
    PUSH target
    jsr rt_peek
    lda rt_depth
    cmp #2
    jne bad
    ldx index
    lda rt_stack+2
    cmp values_lo,x
    jne bad
    lda rt_stack+3
    jne bad
    dec rt_depth
    jsr check_guards
    inc index
    lda index
    cmp #6
    bne @loop
    lda #'K'
    jsr serial_out
    lda #0
    jmp halt
.elseif TEST_CASE = 2
    jsr rt_peek
.elseif TEST_CASE = 3
    jsr rt_poke
.else
    PUSH target
    jsr rt_poke
.endif
bad:
    lda #'F'
    jsr serial_out
    lda #99
    jmp halt

rt_test_probe:
.if TEST_CASE = 4
    lda rt_depth
    cmp #1
    jne bad
    lda rt_stack
    cmp #<target
    jne bad
    lda rt_stack+1
    cmp #>target
    jne bad
.else
    lda rt_depth
    jne bad
.endif
    lda target
    cmp #$a5
    jne bad
    lda guard_before
    cmp #$a5
    jne bad
    lda guard_after
    cmp #$a5
    jne bad
    lda #'K'
    jsr serial_out
    rts

check_guards:
    lda rt_stack
    cmp #$34
    jne bad
    lda rt_stack+1
    cmp #$12
    jne bad
    lda guard_before
    cmp #$a5
    jne bad
    lda guard_after
    cmp #$a5
    jne bad
    rts
