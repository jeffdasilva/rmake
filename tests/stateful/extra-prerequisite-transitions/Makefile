include config.mk
out: input
	@python3 build.py '$<' '$^' '$+' '$?' '$|'
verify: out
	@cat out
