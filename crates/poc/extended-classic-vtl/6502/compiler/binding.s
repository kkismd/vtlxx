; Compile-time Write bindings. Runtime calls contain only resolved targets.
.setcpu "6502"

.export cc_binding_reset, cc_is_bound, cc_publish, cc_resolve
.export cc_name_len, cc_name
.import cc_is_completed
.importzp cc_status, cc_arg

.segment "ZEROPAGE"
binding_ptr: .res 2

.segment "BSS"
; Caller-owned input, valid until the next caller modifies it.
cc_name_len: .res 1
cc_name: .res 8
binding_count: .res 1
binding_remaining: .res 1
; length, eight name bytes, completed target (little endian).
binding_entries: .res 704

.segment "CODE"
cc_binding_reset:
    lda #0
    sta binding_count
    rts

; Input cc_name_len and cc_name. Carry set if found, clear otherwise.
; Invalid input sets cc_status; valid unsuccessful lookup leaves table unchanged.
cc_is_bound:
    jsr binding_find
    rts

; Input name and cc_arg = completed executable target. Publish count last.
cc_publish:
    jsr binding_find
    lda cc_status
    bne @done
    bcc @available
    lda #8
    sta cc_status
    rts
@available:
    lda binding_count
    cmp #64
    bcs @bad
    lda cc_arg
    ldx cc_arg+1
    jsr cc_is_completed
    lda cc_status
    bne @done
    ldy #0
    lda cc_name_len
    sta (binding_ptr),y
    tax
@copy:
    iny
    lda cc_name-1,y
    sta (binding_ptr),y
    dex
    bne @copy
    ldy #9
    lda cc_arg
    sta (binding_ptr),y
    iny
    lda cc_arg+1
    sta (binding_ptr),y
    inc binding_count
@done:
    rts
@bad:
    lda #8
    sta cc_status
    rts

; Returns A/X resolved completed target, or status 8 for unknown name.
cc_resolve:
    jsr binding_find
    lda cc_status
    bne @fail
    bcc @missing
    ; Return low in A and high in X.
    ldy #9
    lda (binding_ptr),y
    pha
    iny
    lda (binding_ptr),y
    tax
    pla
    rts
@missing:
    lda #8
    sta cc_status
@fail:
    lda #0
    tax
    rts

; On success binding_ptr points at matching entry (carry set), otherwise at
; next free entry (carry clear). Pointer advances in 16 bits across pages.
binding_find:
    lda cc_status
    bne @invalid
    lda cc_name_len
    beq @bad
    cmp #9
    bcs @bad
    tax
@validate:
    lda cc_name-1,x
    cmp #'a'
    bcc @bad
    cmp #'z'+1
    bcs @bad
    dex
    bne @validate
    lda #<binding_entries
    sta binding_ptr
    lda #>binding_entries
    sta binding_ptr+1
    lda binding_count
    sta binding_remaining
@entry:
    lda binding_remaining
    beq @missing
    ldy #0
    lda (binding_ptr),y
    cmp cc_name_len
    bne @next
    tax
@compare:
    iny
    lda (binding_ptr),y
    cmp cc_name-1,y
    bne @next
    dex
    bne @compare
    sec
    rts
@next:
    clc
    lda binding_ptr
    adc #11
    sta binding_ptr
    bcc :+
    inc binding_ptr+1
:
    dec binding_remaining
    bne @entry
@missing:
    clc
    rts
@bad:
    lda #8
    sta cc_status
@invalid:
    clc
    rts
