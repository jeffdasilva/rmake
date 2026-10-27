.SHELLFLAGS = $(if $(wildcard trigger),$(error stopped),$(file >trigger,observed)-c)
all: target
target:
	+@:
	@echo forbidden
