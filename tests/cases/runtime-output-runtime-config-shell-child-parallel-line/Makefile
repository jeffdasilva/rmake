SHELL = $(if $(wildcard trigger),$(error stopped),/bin/sh)
all: target
target:
	+@touch trigger
	@echo forbidden
