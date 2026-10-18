INNER = $(info export)$(warning export)value
export OUTER = $(INNER)
all:
	@printf 'child=<%s>\n' "$$OUTER"
