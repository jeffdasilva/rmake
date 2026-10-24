.SECONDEXPANSION:
all: one.out
%.out: $$(info OUT=$$*)%.mid $$(info NEVER)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
%.mid: $$(info MID=$$*)missing
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
