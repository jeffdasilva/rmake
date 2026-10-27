IFS = $(if $(wildcard trigger),$(error stopped),)
all: target
target:
	+@$(file >trigger,observed)
	@echo forbidden
