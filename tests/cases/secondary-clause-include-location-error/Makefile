.SECONDEXPANSION:
include rules.mk
all: $$(info last @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] ?=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$?"
