$(shell python3 setup.py)
.LIBPATTERNS=lib%.a lib%.a
all: -ldemo;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
