FIRST := $(wildcard lib.a(m029?.o))
$(file >lib.a,invalid)
SECOND := $(wildcard lib.a(m029?.o))
X := $(shell cp replacement.a lib.a)
$(info before=[$(FIRST)] after=[$(SECOND)])
all: lib.a(new99.o)
	@printf "first=[%s] all=[%s]\n" "$<" "$^"
