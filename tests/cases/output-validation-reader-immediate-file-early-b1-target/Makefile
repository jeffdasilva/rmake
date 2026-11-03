$(file >seen,anchor)
MAKEFLAGS = --output-sync=badÿ A:::=$$(file\ >>seen,A)oneÿ B:::=$$(file\ >>seen,B)twoÿ CHECK:=$$(file\ >>seen,DONE)$$(info\ VALUES\ A=[$$(value\ A)]/$$(origin\ A)/$$(flavor\ A)\ B=[$$(value\ B)]/$$(origin\ B)/$$(flavor\ B))
all:;@$(file >>seen,END)true
