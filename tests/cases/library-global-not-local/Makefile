.LIBPATTERNS=lib%.a
all: .LIBPATTERNS=$(error LOCAL)
$(shell python3 setup.py)
all: -ldemo;@printf '[%s|%s|%s]\n' '$<' '$(origin .LIBPATTERNS)' '$(flavor .LIBPATTERNS)'
