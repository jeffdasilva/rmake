.SECONDEXPANSION:
all: one.out
vpath %.in inputs
%.out: $$(info scan)%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
