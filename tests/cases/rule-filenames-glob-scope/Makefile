$(shell python3 setup.py)
*.out: X=local
.PHONY: all a.out b.out
all: a.out b.out
*.out:;@printf "[%s|%s]\n" '$@' '$(X)'
