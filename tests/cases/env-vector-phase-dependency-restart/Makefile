$(shell cc -O2 -o capture capture.c)
A=file
unused: .EXPORT_ALL_VARIABLES
$(info reader=[$(shell ./capture A)])
include made.mk
all: ; @./capture A
made.mk: ; @./capture A > observed; printf "READY=yes\n" > $@
