$(file >seen,anchor)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)),$(file >>seen,NEXT-$(if $(value GNUMAKEFLAGS),present,cleared))) -Otarget
GNUMAKEFLAGS = READY:=$$(file\ >>seen,READY)yesÿ --output-sync=badÿ CHECK:=$$(file\ >>seen,DONE)$$(info\ READY=[$$(value\ READY)]/$$(origin\ READY)) --output-sync=none
all:;@$(file >>seen,END)true
