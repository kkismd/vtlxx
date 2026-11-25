; Compile and runtime views have disjoint lifetimes. Persistent status and
; driver entry, and cc65 library zero page, are reserved separately.
; Compile: basic 0..39, source 40..42, arena 43..61, emitter 62..66,
; owner 67..79. These ranges may be live across calls.
; Runtime: state 0..92, primitives 93..99; neither overlaps compilation.
; Shared arithmetic operands/result 85..90 are live only for each arithmetic
; call in either phase; compile live state ends at 79. Their existing aliases
; and offsets remain fixed (emitter embeds the rt_lhs address).
; Keep the corresponding private aliases in each module within these ranges.
.exportzp zp_phase
.segment "ZEROPAGE"
zp_phase: .res 100
