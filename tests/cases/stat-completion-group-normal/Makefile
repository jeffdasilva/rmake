$(shell python3 setup.py)
.PHONY: trigger
out: a b;@printf 'parent=[%s]\n' '$?'
a b &: trigger;
