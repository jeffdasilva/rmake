VPATH = src
all: foo bar
	@echo all=$^
src/foo bar &: input
	@echo group=$@:$^; cp -p input src/foo; cp -p input bar
