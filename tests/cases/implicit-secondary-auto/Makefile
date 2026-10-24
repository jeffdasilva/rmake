.SECONDEXPANSION:
all: one.out
one.out: explicit
%.out: $$(info AT=[$$@] STAR=[$$*] FIRST=[$$<] ALL=[$$^] PLUS=[$$+] ORDER=[$$|])%.in $$(info AFTER=[$$^])
	@echo "$@:$*:$<:$^"
explicit: ;
