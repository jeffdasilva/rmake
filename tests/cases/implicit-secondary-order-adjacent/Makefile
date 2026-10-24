.SECONDEXPANSION:
all: one.out
%.out: $$(strip one.in|order) later
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
order later: ;
