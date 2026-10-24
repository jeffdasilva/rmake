.DEFAULT_GOAL:=all
.SECONDEXPANSION:
-include rules.mk
rules.mk: template ; @cp $< $@
all: item.o
item.o: %.o: $$(info A=$$* restart=$$(MAKE_RESTARTS))one
item.o: i%.o: $$(info B=$$* restart=$$(MAKE_RESTARTS))two
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|]"
one two three: ;
