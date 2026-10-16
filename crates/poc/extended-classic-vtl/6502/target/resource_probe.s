; Test-only resource report entry. This is linked only by the ignored host test.
.setcpu "6502"

.export _main
.export ru_value_stack_peak, ru_storage_addr, ru_storage_extent
.export ru_storage_unique, ru_work_stack_peak, ru_label_peak, ru_fixup_peak
.export ru_owner_completions, ru_write_publishes, ru_template_publishes
.export ru_structured_peak, ru_while_peak, ru_chunk_peak, ru_constant_peak
.export ru_hw_peak
.export ru_storage_seen
.export ru_patch_entries_peak

.import fe_compile_run
.importzp fe_compile_status
.import cc_arena, cc_arena_end
.importzp cc_cursor, cc_source_floor, cc_pending_count
.import serial_out, halt

.segment "BSS"
ru_value_stack_peak: .res 1
ru_storage_addr:    .res 1
ru_storage_extent:  .res 1
ru_storage_unique:  .res 1
ru_work_stack_peak: .res 1
ru_label_peak:      .res 1
ru_fixup_peak:      .res 1
ru_owner_completions: .res 1
ru_write_publishes: .res 1
ru_template_publishes: .res 1
ru_patch_entries_peak: .res 2
ru_structured_peak: .res 1
ru_while_peak:      .res 1
ru_chunk_peak:      .res 1
ru_constant_peak:   .res 1
ru_hw_peak:         .res 1
ru_hw_base:         .res 1
ru_hw_depth:        .res 1
ru_code_bytes:      .res 2
ru_source_bytes:    .res 2
ru_exit_status:     .res 1
ru_zero_index:      .res 1
ru_storage_seen: .res 128

.segment "CODE"
_main:
    jsr ru_reset
    ; Fill the free portion of the NMOS 6502 hardware stack with an
    ; address-dependent canary. The incoming _main return address is above SP.
    tsx
    stx ru_hw_base
    ldx #0
@seed_stack:
    txa
    eor #RESOURCE_CANARY
    sta $0100,x
    cpx ru_hw_base
    beq @stack_seeded
    inx
    bne @seed_stack
@stack_seeded:
    jsr fe_compile_run
    bcc @compile_ok
    lda fe_compile_status
    ora #$80
    sta ru_exit_status
    jmp @measure_stack
@compile_ok:
    lda #0
    sta ru_exit_status

@measure_stack:
    ; Scan after fe_compile_run returned, before reporting adds any calls.
    ldx ru_hw_base
    lda #0
    sta ru_hw_peak
@scan_stack:
    txa
    eor #RESOURCE_CANARY
    cmp $0100,x
    beq @scan_next
    ; Recompute the depth as base - X + 1.
    txa
    sta ru_zero_index
    sec
    lda ru_hw_base
    sbc ru_zero_index
    clc
    adc #1
    cmp ru_hw_peak
    bcc @scan_next
    sta ru_hw_peak
@scan_next:
    cpx #0
    beq @stack_measured
    dex
    jmp @scan_stack
@stack_measured:
    jsr ru_capture_storage
    jsr ru_capture_arena
    jmp ru_report

ru_reset:
    lda #0
    sta ru_value_stack_peak
    sta ru_storage_addr
    sta ru_storage_extent
    sta ru_storage_unique
    sta ru_work_stack_peak
    sta ru_label_peak
    sta ru_fixup_peak
    sta ru_owner_completions
    sta ru_write_publishes
    sta ru_template_publishes
    sta ru_patch_entries_peak
    sta ru_patch_entries_peak+1
    sta ru_structured_peak
    sta ru_while_peak
    sta ru_chunk_peak
    sta ru_constant_peak
    sta ru_hw_peak
    sta ru_zero_index
    ldx #127
@clear_storage_seen:
    sta ru_storage_seen,x
    dex
    bpl @clear_storage_seen
    rts

ru_capture_arena:
    sec
    lda cc_cursor
    sbc #<cc_arena
    sta ru_code_bytes
    lda cc_cursor+1
    sbc #>cc_arena
    sta ru_code_bytes+1
    sec
    lda #<cc_arena_end
    sbc cc_source_floor
    sta ru_source_bytes
    lda #>cc_arena_end
    sbc cc_source_floor+1
    sta ru_source_bytes+1
    rts

ru_capture_storage:
    ldx #0
@scan_storage:
    lda ru_storage_seen,x
    beq @storage_next
    inc ru_storage_unique
    txa
    clc
    adc #1
    cmp ru_storage_extent
    bcc @storage_next
    sta ru_storage_extent
@storage_next:
    inx
    cpx #128
    bne @scan_storage
    rts

ru_report:
    lda #'R'
    jsr ru_emit
    lda #'U'
    jsr ru_emit
    lda #'1'
    jsr ru_emit
    lda ru_code_bytes
    jsr ru_emit
    lda ru_code_bytes+1
    jsr ru_emit
    lda ru_source_bytes
    jsr ru_emit
    lda ru_source_bytes+1
    jsr ru_emit
    lda ru_value_stack_peak
    jsr ru_emit
    lda ru_hw_peak
    jsr ru_emit
    lda ru_storage_unique
    jsr ru_emit
    lda ru_storage_extent
    jsr ru_emit
    lda ru_work_stack_peak
    jsr ru_emit
    lda ru_label_peak
    jsr ru_emit
    lda ru_fixup_peak
    jsr ru_emit
    lda ru_structured_peak
    jsr ru_emit
    lda ru_while_peak
    jsr ru_emit
    lda ru_chunk_peak
    jsr ru_emit
    lda ru_constant_peak
    jsr ru_emit
    lda ru_owner_completions
    jsr ru_emit
    lda ru_write_publishes
    jsr ru_emit
    lda ru_template_publishes
    jsr ru_emit
    lda ru_patch_entries_peak
    jsr ru_emit
    lda ru_patch_entries_peak+1
    jsr ru_emit
    lda ru_exit_status
    jmp halt

ru_emit:
    jmp serial_out
