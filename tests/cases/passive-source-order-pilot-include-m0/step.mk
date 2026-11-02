override GNUMAKEFLAGS = READY:=yes STATE:=$$(if\ $$(value\ GNUMAKEFLAGS),pending,cleared) A:=$$(file\ >>seen,A)$$(info\ GNU_A)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)), $(file >>seen,FLAGS)$(info FLAGS_STATE=[$(STATE)] A=[$(A)]) B:=two)
