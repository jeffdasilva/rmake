.SECONDEXPANSION:
all: first.out second.out
%.out: d/new.in
	@echo first=$@:$<
%.out: $$(file >d/new.in,created)$$(info names=$$(wildcard d/*.in))
	@echo second=$@
