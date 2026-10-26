$(shell python3 setup.py)
.LIBPATTERNS=$(error PATTERNS)
all: -ldemo;@printf 'target=[%s] first=[%s] all=[%s] newer=[%s]\n' '$@' '$<' '$^' '$?'
