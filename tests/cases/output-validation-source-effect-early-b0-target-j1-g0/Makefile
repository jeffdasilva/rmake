$(file >seen,anchor)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)),$(file >>seen,NEXT-$(if $(value GNUMAKEFLAGS),present,cleared))) -Otarget
GNUMAKEFLAGS = --output-sync=bad READY:=$$(file\ >>seen,READY)yes CHECK:=$$(file\ >>seen,DONE)$$(info\ READY=[$$(value\ READY)]/$$(origin\ READY))
all:;@$(file >>seen,END)true
