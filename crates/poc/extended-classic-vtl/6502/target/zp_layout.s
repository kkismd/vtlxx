; Compile and runtime views have disjoint lifetimes. Persistent status and
; driver entry, and cc65 library zero page, are reserved separately.
; Offsets in the compile view: basic 0..45, source 46..48, template 49,
; reader 50..56, arena 57..88, emitter 89..93, owner 94..106,
; work stack 108..110; 107 is unused. Runtime: state 0..92, primitives 93..99.
; Keep the corresponding private aliases in each module within these ranges.
.exportzp zp_phase
.segment "ZEROPAGE"
zp_phase: .res 111
