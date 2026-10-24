.SECONDEXPANSION:
all: one.out
one.out: $$(info explicit=$$^)$$(INPUT)
INPUT=extra
%.out: $$(info implicit=$$^)%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
extra: ;
