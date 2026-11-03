$(file >seen,anchor)
.SECONDEXPANSION:
ACTION = $(eval MAKEFLAGS = --output-sync=bad A:=$$$$(file\ >>seen,A)one B:=$$$$(file\ >>seen,B)two CHECK:=$$$$(file\ >>seen,DONE)$$$$(info\ VALUES\ A=[$$$$(value\ A)]/$$$$(origin\ A)/$$$$(flavor\ A)\ B=[$$$$(value\ B)]/$$$$(origin\ B)/$$$$(flavor\ B)))
all: $$(ACTION)
	@$(file >>seen,END)true
