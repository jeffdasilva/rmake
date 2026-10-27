.SHELLFLAGS=$(if $(wildcard trigger),$(error stopped),-c)
.INTERMEDIATE:mid
all:built .WAIT target
built:mid;@true
mid:;@touch mid
target:
	@touch trigger
	@echo forbidden
