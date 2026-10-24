.SECONDEXPANSION:
all: dir/one.out
%.out: $$(addprefix %,.in .src)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
