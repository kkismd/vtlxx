; Compile and runtime views have disjoint lifetimes. Persistent status and
; driver entry, and cc65 library zero page, are reserved separately.
; Compile: basic 0..39, source 40..42, arena 43..61, emitter 62..66,
; owner 67..79, work stack 80..82. These ranges may be live across calls.
; Runtime: state 0..92, primitives 93..99; neither overlaps compilation.
; Keep the corresponding private aliases in each module within these ranges.
.exportzp zp_phase
.segment "ZEROPAGE"
zp_phase: .res 100
