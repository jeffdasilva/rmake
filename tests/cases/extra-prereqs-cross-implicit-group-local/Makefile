all:a.out a.peer
%.out %.peer:;@printf 'group=[%s|%s]\n' '$<' '$^'
a.out:.EXTRA_PREREQS=x
a.peer:.EXTRA_PREREQS=y
x:;@echo x
y:;@echo y
