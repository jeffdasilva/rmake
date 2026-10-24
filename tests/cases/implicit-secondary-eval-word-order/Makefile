.SECONDEXPANSION:
all: one.out
%.out: $$(eval V=chosen)$$(V).in $$(info V=$$(V))
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
