.SHELLFLAGS=$(if $(wildcard trigger),$(error stopped),-c)
-include generated.mk
generated.mk:
	@printf "#generated\n" >generated.mk;touch trigger
	@echo forbidden
all:;@echo goal-forbidden
