$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
.PHONY: force
all: -ldemo;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
-ldemo: force;@printf 'recipe=[%s]\n' '$@'
