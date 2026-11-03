$(file >seen,anchor)
GNUMAKEFLAGS = A?=$$(file\ >>seen,A)oneÿ B?=$$(file\ >>seen,B)twoÿ CHECK:=$$(file\ >>seen,DONE)$$(eval\ unexport\ A\ B)$$(info\ VALUES\ A=[$$(value\ A)]/$$(origin\ A)/$$(flavor\ A)\ B=[$$(value\ B)]/$$(origin\ B)/$$(flavor\ B)) --output-sync=badÿ
all:;@$(file >>seen,END)true
