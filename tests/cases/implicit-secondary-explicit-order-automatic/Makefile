.SECONDEXPANSION:
all: one.out
one.out: two two | two order
%.out: $$(info HERE <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
two order: ;
