.SECONDEXPANSION:
all: dir/one.out
%.out: $$(if %,/dev/null)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
