$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
.SECONDEXPANSION:
P=-ldemo
all: $$(P);@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
