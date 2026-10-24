.SECONDEXPANSION:
all: one.out
%.out:: $$(info terminal=$$*)%.mid
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
%.mid: %.in ; @touch $@
