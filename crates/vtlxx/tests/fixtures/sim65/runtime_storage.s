.setcpu "6502"
.macpack longbranch
.export _main
.import rt_init, rt_push, rt_load_storage, rt_store_storage
.import rt_load_reg, rt_store_reg, halt
.importzp rt_depth, rt_stack, rt_input_eof
.include "runtime_macros.inc"

.segment "BSS"
index: .res 1
round: .res 1

.segment "CODE"
_main:
    jsr rt_init
    lda #2
    sta round
@round:
    ; Fill all 256 cells with distinct two-byte patterns.
    lda #0
    sta index
@fill:
    lda index
    ldx #0
    jsr rt_push
    lda index
    eor #$a5
    ldx index
    jsr rt_push
    jsr rt_store_storage
    inc index
    bne @fill

    ; Read all cells, including 0/128 and 127/128, with a prefix
    ; left on the value stack and an unrelated register and EOF flag.
    PUSH 1234
    PUSH 5678
    ldx #7
    jsr rt_store_reg
    lda #1
    sta rt_input_eof
    lda #0
    sta index
@check:
    lda index
    ldx #0
    jsr rt_push
    jsr rt_load_storage
    lda rt_depth
    cmp #2
    jne @fail
    lda rt_stack+2
    eor #$a5
    cmp index
    jne @fail
    lda rt_stack+3
    cmp index
    jne @fail
    dec rt_depth
    inc index
    bne @check

    ; High index bits are ignored: -1=255, 256=0, 257=1.
    PUSH -1
    jsr rt_load_storage
    lda rt_stack+2
    cmp #$5a
    jne @fail
    lda rt_stack+3
    cmp #$ff
    jne @fail
    dec rt_depth
    PUSH 256
    jsr rt_load_storage
    lda rt_stack+2
    cmp #$a5
    jne @fail
    lda rt_stack+3
    jne @fail
    dec rt_depth
    PUSH 257
    jsr rt_load_storage
    lda rt_stack+2
    cmp #$a4
    jne @fail
    lda rt_stack+3
    cmp #1
    jne @fail
    dec rt_depth
    ldx #7
    jsr rt_load_reg
    lda rt_stack+2
    cmp #<5678
    jne @fail
    lda rt_stack+3
    cmp #>5678
    jne @fail
    lda rt_input_eof
    cmp #1
    jne @fail
    lda rt_stack
    cmp #<1234
    jne @fail
    lda rt_stack+1
    cmp #>1234
    jne @fail

    ; The same aliases apply to writes as well as reads.
    PUSH -1
    PUSH 1111
    jsr rt_store_storage
    PUSH 256
    PUSH 2222
    jsr rt_store_storage
    PUSH 257
    PUSH 3333
    jsr rt_store_storage
    PUSH 255
    jsr rt_load_storage
    lda rt_stack+4
    cmp #<1111
    jne @fail
    lda rt_stack+5
    cmp #>1111
    jne @fail
    dec rt_depth
    PUSH 0
    jsr rt_load_storage
    lda rt_stack+4
    cmp #<2222
    jne @fail
    lda rt_stack+5
    cmp #>2222
    jne @fail
    dec rt_depth
    PUSH 1
    jsr rt_load_storage
    lda rt_stack+4
    cmp #<3333
    jne @fail
    lda rt_stack+5
    cmp #>3333
    jne @fail

    ; Reinitialize and inspect every byte through the public load helper.
    jsr rt_init
    lda #0
    sta index
@zero:
    lda index
    ldx #0
    jsr rt_push
    jsr rt_load_storage
    lda rt_stack
    ora rt_stack+1
    jne @fail
    dec rt_depth
    inc index
    bne @zero
    dec round
    beq @done
    jmp @round
@fail:
    lda #9
    jmp halt
@done:
    DONE
