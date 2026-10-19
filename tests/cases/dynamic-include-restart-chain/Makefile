.DEFAULT_GOAL:=all
$(info pass=$(MAKE_RESTARTS))
-include first.mk
all:;@echo $(V)
first.mk:;@printf "include second.mk\n" > $@
second.mk:;@echo V=complete > $@
