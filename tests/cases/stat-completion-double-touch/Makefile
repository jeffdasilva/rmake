$(shell python3 setup.py)
.PHONY: trigger
out: child;@printf 'parent=[%s]\n' '$?'
child:: trigger;
child::;
