.SHELLFLAGS = $(if $(wildcard trigger),$(error stopped),-c)
all: target
target:
	+@$(file >trigger,observed)
	@echo forbidden
