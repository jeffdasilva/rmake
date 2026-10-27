.PHONY: all a b c
all: a b c
	@cat order
a:
	@$(file >expanded-a)python3 helper.py a
b:
	@$(file >expanded-b)python3 helper.py b
c:
	@$(file >expanded-c)python3 helper.py c
