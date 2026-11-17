-include opts.mk
all: out
	@printf "parent\n"
out: seed
	cp seed out
opts.mk: seed
	$(info BEFORE=[$(MAKEFLAGS)])$(info AFTER=[$(MAKEFLAGS)])@printf "GEN=yes\n" >opts.mk
