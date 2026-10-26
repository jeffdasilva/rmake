P=old.in
out: $(P)
ifeq ($(eval P=new.in),)
endif
	@printf '[%s|%s|%s]\n' '$@' '$^' '$|'
old.in new.in:;@:
