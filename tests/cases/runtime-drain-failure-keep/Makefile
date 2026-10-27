IFS=$(if $(and $(filter b,$@),$(wildcard trigger)),$(error second stopped),)
.SHELLFLAGS=-c
all:b a
a:
	@python3 barrier.py a; exit 3
	@printf forbidden
b:
	+@python3 barrier.py b
	@printf "b-finished\n"
