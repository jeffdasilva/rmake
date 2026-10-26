.EXTRA_PREREQS=hidden
.DEFAULT_GOAL=all
all: normal | hidden;@printf '[%s|%s|%s|%s|%s]\n' '$<' '$^' '$+' '$?' '$|'
normal:;@printf 'normal\n'
hidden:;@printf 'hidden\n'
other:;@printf 'other\n'
