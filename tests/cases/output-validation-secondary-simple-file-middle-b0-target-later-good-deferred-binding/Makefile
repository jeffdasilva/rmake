$(file >seen,anchor)
.SECONDEXPANSION:
ACTION = $(eval MAKEFLAGS = A:=$$$$(file\ >>seen,A)one --output-sync=bad B:=$$$$(file\ >>seen,B)two CHECK:=$$$$(file\ >>seen,DONE)$$$$(info\ VALUES\ A=[$$$$(value\ A)]/$$$$(origin\ A)/$$$$(flavor\ A)\ B=[$$$$(value\ B)]/$$$$(origin\ B)/$$$$(flavor\ B)) --output-sync=none)
all: $$(ACTION)
	@$(file >>seen,END)true
