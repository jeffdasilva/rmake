export X=global
%.out: private X=local
one.out:;@echo $(shell printf %s "$$X"):$(X)
