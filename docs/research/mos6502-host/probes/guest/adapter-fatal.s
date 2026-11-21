.setcpu "6502"
.export serial_in, serial_try_in, serial_out, halt, entropy16
.import _putchar, exit, rt_input_failure
.segment "CODE"
serial_in:
 jsr $fff0
 cpx #0
 beq byte
 cpx #2
 beq eof
 jmp rt_input_failure
byte: clc
 rts
eof: sec
 rts
serial_try_in:
 jmp $ffef
serial_out:
 ldx #0
 jmp _putchar
halt:
 jmp exit
entropy16:
 sec
 rts
