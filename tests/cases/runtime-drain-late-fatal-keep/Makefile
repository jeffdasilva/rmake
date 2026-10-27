IFS=$(if $(and $(filter b,$@),$(wildcard trigger)),$(error second stopped),)
.SHELLFLAGS=$(if $(and $(filter a,$@),$(wildcard trigger)),$(file >fatal-ready,seen)$(error first stopped),-c)
all:b a
a:
	@python3 barrier.py a
	@printf forbidden
b:
	+@python3 barrier.py b
	@$(if $(wildcard trigger),,echo b)
