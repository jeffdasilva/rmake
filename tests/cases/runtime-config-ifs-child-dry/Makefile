IFS = $(if $(wildcard trigger),$(error stopped),)
all: target
target:
	+@touch trigger
	@echo forbidden
