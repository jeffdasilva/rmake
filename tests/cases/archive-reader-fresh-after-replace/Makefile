FIRST := $(wildcard lib.a(m029?.o))
X := $(shell cp replacement.a lib.a)
$(info before=[$(FIRST)] after=[$(wildcard lib.a(new*.o))])
all: lib.a(new99.o)
	@printf "first=[%s] all=[%s]\n" "$<" "$^"
