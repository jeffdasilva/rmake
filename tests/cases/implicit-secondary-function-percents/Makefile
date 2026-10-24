.SECONDEXPANSION:
all: one.out
%.out: $$(patsubst %.in,%.src,$$*.in)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
