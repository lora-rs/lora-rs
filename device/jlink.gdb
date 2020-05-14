target extended-remote :3333

# set print asm-demangle on

# detect unhandled exceptions, hard faults and panics
break HardFault
break rust_begin_unwind

# *try* to stop at the user entry point (it might be gone due to inlining)
break main

#monitor arm semihosting enable
#monitor semihosting ioclient 3

load

# start the process but immediately halt the processor
stepi
