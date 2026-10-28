$(shell cc -O2 -o capture capture.c; chmod +x wrapper)
$(shell mkdir denied; cp wrapper denied/wrapper; chmod -x denied/wrapper)
export PATH=denied:.:/bin:/usr/bin
export A=one
all: ; @wrapper
