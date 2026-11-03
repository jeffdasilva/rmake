$(file >seen,anchor)
MAKEFLAGS = A:=$$(file\ >>seen,A)one --output-sync=bad --output-sync=none B:=$$(file\ >>seen,B)two
all:;@$(file >>seen,END)true
