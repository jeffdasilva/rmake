.DEFAULT_GOAL=out
$(shell python3 setup.py)
out: archive.a(member);@printf "rebuilt=[%s]\n" "$?"
archive.a(member): change;@$(info observed)
.PHONY:change
change:;@cp invalid.a archive.a
