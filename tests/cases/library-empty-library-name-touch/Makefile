$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
all: -l;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
