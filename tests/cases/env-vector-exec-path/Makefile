$(shell cc -O2 -o capture capture.c; chmod +x wrapper)
export PATH=.:/bin:/usr/bin
export A=one
all: ; @wrapper
