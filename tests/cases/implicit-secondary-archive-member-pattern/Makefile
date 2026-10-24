.SECONDEXPANSION:
all: lib.a(one.o)
(%.o): $$(info AT=$$@ MEMBER=$$% STEM=$$*)%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
