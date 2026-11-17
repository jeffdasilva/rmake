-include opts.mk
all: out
	@printf "parent\n"
out: seed
	cp seed out
opts.mk: seed
	$(eval MAKEFLAGS = -q)@printf "GEN=yes\n" >opts.mk
