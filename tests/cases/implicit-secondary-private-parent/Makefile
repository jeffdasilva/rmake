.SECONDEXPANSION:
all: private V=private
all: one.out
%.out: $$(info V=[$$(V)])%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
