$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
vpath lib%.a a
vpath lib%.so b
all: -ldemo;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
