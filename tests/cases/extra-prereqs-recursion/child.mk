.EXTRA_PREREQS=inner
all:;@printf 'child=[%s|%s]\n' '$<' '$^'
inner:;@echo inner
