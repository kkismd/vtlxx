; Compile-session fatal diagnostics and non-local return.
.setcpu "6502"
.export session_abort, session_backend_error
.exportzp fe_compile_status, session_entry_sp
.importzp cc_status
.segment "ZEROPAGE"
fe_compile_status: .res 1
session_entry_sp: .res 1
.code
session_backend_error:
    sta cc_status
    lda #4
    sta fe_compile_status
session_abort:
    ldx session_entry_sp
    txs
    sec
    rts
