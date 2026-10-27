.SHELLFLAGS=$(if $(wildcard trigger),$(error stopped),-c)
include generated.mk
first.mk:;@printf "#first\n" >first.mk
generated.mk:first.mk
	@printf "#generated\n" >generated.mk;touch trigger
	@echo forbidden
all:;@echo goal-forbidden
