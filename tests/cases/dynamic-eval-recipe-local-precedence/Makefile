V=global
all: first second
first: V=local
first:;@echo $(eval V=updated)$(V)
second:;@echo $(V)
