.EXTRA_PREREQS=$(info G[$(MAKE_RESTARTS)])
-include extra.mk
extra.mk:;@echo "X=1" >$@
.DEFAULT_GOAL=all
all: normal;@printf '[%s|%s|%s|%s|%s]\n' '$<' '$^' '$+' '$?' '$|'
normal:;@printf 'normal\n'
hidden:;@printf 'hidden\n'
other:;@printf 'other\n'
