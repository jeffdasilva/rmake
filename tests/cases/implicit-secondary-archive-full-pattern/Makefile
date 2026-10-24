.SECONDEXPANSION:
all: lib.a(dir/one.o)
lib.a(%.o): $$(info AT=$$@ MEMBER=$$% STEM=$$*)%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
