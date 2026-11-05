$(file >>seen,reader)
override GNUMAKEFLAGS = $(file >>seen,gnu-source)$(info gnu-source)GO:=yes --sync-mutex=fnm:absent FIRST:=$$(file\ >>seen,gnu-payload)
override MAKEFLAGS = $(if $(filter environment,$(origin GO)),$(file >>seen,make-source)$(info make-source) --sync-mutex=fnm:lock SECOND:=$$(file\ >>seen,make-payload))
$(file >>seen,reader-end)
all:;@$(file >>seen,recipe)$(info recipe):
