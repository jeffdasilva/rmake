.SECONDEXPANSION:
all: one.out
%.out: $$(info scan)%.in | order later
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
order later: ;
