.SECONDEXPANSION:
all: one.out
MEMBERS=a b
%.out: lib.a($$(MEMBERS))
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
lib.a(a b): ;
