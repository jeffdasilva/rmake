all:a b
a b &:;@printf 'group=[%s|%s]\n' '$<' '$^'
a:.EXTRA_PREREQS=x
b:.EXTRA_PREREQS=y
x:;@echo x
y:;@echo y
