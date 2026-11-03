$(file >seen,anchor)
GNUMAKEFLAGS = A:=$$(file\ >>seen,A)one B:=$$(file\ >>seen,B)two CHECK:=$$(file\ >>seen,DONE)$$(eval\ unexport\ A\ B)$$(info\ VALUES\ A=[$$(value\ A)]/$$(origin\ A)/$$(flavor\ A)\ B=[$$(value\ B)]/$$(origin\ B)/$$(flavor\ B)) --output-sync=bad
all:;@$(file >>seen,END)true
