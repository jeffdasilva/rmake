$(shell python3 setup.py)
all: dep
	@printf 'ALL=[%s]\n' '$?'
dep::
	@python3 backdate.py
