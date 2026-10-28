guile=GLOBAL:$(1)
all: first second
first: guile=FIRST:$(1)
first second: ; @$(info $@=[$(call guile,ARG)])true
