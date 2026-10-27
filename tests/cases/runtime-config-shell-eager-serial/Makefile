SHELL = $(if $(wildcard trigger),$(error stopped),/bin/sh)
all: target
target:
	+@$(file >trigger,observed)
	@echo forbidden
