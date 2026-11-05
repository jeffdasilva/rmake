.PHONY: all
all:
	@python3 -B witness.py '$(if $(filter --jobserver-auth=%,$(MAKEFLAGS)),yes,no)'
