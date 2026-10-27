.SHELLFLAGS=$(if $(wildcard trigger),$(error stopped),-c)
.INTERMEDIATE:mid
all:built .WAIT target
built:mid;@true
mid:;@touch mid
target:
	@$(file >trigger,observed)
	@echo forbidden
