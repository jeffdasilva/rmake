$(shell cc -O2 -o capture capture.c)
A=file
.EXPORT_ALL_VARIABLES:
$(info reader=[$(shell ./capture A)])
all: ; @$(eval export)./capture A
