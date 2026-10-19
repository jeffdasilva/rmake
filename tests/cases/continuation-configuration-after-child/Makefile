.SHELLFLAGS = $(if $(wildcard trigger),$(error stopped),-c)
all: target
target:
	@touch trigger
	@echo unexpected
