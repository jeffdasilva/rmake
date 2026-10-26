.EXTRA_PREREQS=$(info EXTRA)hidden
.DEFAULT_GOAL=all
all: normal;@printf '[%s|%s|%s|%s|%s]\n' '$<' '$^' '$+' '$?' '$|'
normal:;@printf 'normal\n'
hidden:;@printf 'hidden\n'
other:;@printf 'other\n'
