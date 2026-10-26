$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
VPATH=d
GPATH=d
all: -ldemo;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
