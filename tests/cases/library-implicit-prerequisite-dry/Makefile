$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
.DEFAULT_GOAL=demo.out
%.out: -l%;@printf 'first=[%s]\n' '$<'
