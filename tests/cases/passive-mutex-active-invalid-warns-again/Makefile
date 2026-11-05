$(file >>seen,reader)
override GNUMAKEFLAGS = $(file >>seen,gnu-source)$(info gnu-source)GO:=yes --output-sync=target --sync-mutex=broken
override MAKEFLAGS = $(if $(filter environment,$(origin GO)),$(file >>seen,make-source)$(info make-source))
all:;@$(file >>seen,recipe)$(info recipe):
