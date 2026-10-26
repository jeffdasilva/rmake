$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
vpath lib%.so a
vpath lib%.a b
all: -ldemo;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
