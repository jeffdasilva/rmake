.SHELLFLAGS=-c
all:b a
b:
	+@python3 barrier.py b
	@printf "b-finished\n"
a:ready;@$(file >fatal-ready,seen)$(error first stopped)
ready:
	@python3 barrier.py a
