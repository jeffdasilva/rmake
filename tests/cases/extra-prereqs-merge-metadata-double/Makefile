.DEFAULT_GOAL=all
VPATH=d
all:input;@echo done
input::visible
d/input:.EXTRA_PREREQS=hidden
visible:;@echo visible
hidden:;@echo hidden
input::other;@printf 'second=[%s|%s]\n' '$@' '$^'
other:;@echo other
