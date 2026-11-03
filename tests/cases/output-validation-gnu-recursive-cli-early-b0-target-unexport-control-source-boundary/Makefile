$(file >seen,anchor)
GNUMAKEFLAGS = --output-sync=bad A=$$(file\ >>seen,A)one B=$$(file\ >>seen,B)two CHECK:=$$(file\ >>seen,DONE)$$(eval\ unexport\ A\ B)$$(info\ VALUES\ A=[$$(value\ A)]/$$(origin\ A)/$$(flavor\ A)\ B=[$$(value\ B)]/$$(origin\ B)/$$(flavor\ B))
all:;@$(file >>seen,END)true
