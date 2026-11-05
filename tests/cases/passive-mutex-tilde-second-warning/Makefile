$(file >>seen,reader)
override GNUMAKEFLAGS = $(file >>seen,gnu-source)$(info gnu-source)GO:=yes --sync-mutex=fnm:lock FIRST:=$$(file\ >>seen,gnu-payload)
override MAKEFLAGS = $(if $(filter environment,$(origin GO)),$(file >>seen,make-source)$(info make-source) -I~/unused --sync-mutex=broken SECOND:=$$(file\ >>seen,make-payload))
$(file >>seen,reader-end)
all:;@$(file >>seen,recipe)$(info recipe):
