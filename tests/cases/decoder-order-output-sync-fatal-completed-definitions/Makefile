$(file >seen,anchor)
MAKEFLAGS = A:=$$(file\ >>seen,A)one --output-sync=bad B:=$$(file\ >>seen,B)two
all:;@$(file >>seen,END)true
