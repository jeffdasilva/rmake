$(file >seen,anchor)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)),$(file >>seen,NEXT-$(if $(value GNUMAKEFLAGS),present,cleared))) -Otarget
GNUMAKEFLAGS = --output-sync=badÿ READY:=$$(file\ >>seen,READY)yesÿ CHECK:=$$(file\ >>seen,DONE)$$(info\ READY=[$$(value\ READY)]/$$(origin\ READY))
all:;@$(file >>seen,END)true
